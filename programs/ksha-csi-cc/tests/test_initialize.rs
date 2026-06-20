use {
    anchor_lang::{InstructionData, ToAccountMetas, solana_program::{instruction::Instruction, pubkey::Pubkey}}, 
     litesvm::LiteSVM, solana_keypair::Keypair, solana_message::{ Message, VersionedMessage}, 
    solana_signer::Signer, solana_transaction::versioned::VersionedTransaction
};

use spl_associated_token_account::{ get_associated_token_address, instruction::create_associated_token_account};
use anchor_lang::solana_program::program_pack::Pack;



#[test]
fn test_initialize() {
    let program_id = ksha_csi_cc::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/ksha_csi_cc.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let mint_keypair = Keypair::new();
    let mint_pubkey = Signer::pubkey(&mint_keypair);
    
    // Derive the two PDAs the program expects — must match the seeds
    // in lib.rs exactly: [b"platform_state"] and [b"platform_authority"]
    let (platform_state, _state_bump) =
        Pubkey::find_program_address(&[b"platform_state"], &program_id);
    let (platform_authority, _authority_bump) =
        Pubkey::find_program_address(&[b"platform_authority"], &program_id);
 
    let instruction = Instruction::new_with_bytes(
        program_id,
        &ksha_csi_cc::instruction::InitPlatform {
            admin: payer.pubkey(),
        }.data(),
        ksha_csi_cc::accounts::InitPlatform {
            payer: payer.pubkey(),
            platform_state,
            platform_authority,
            mint: mint_pubkey,
            token_program: anchor_spl::token::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );

    println!("Mint: {}, payer: {}, platform state: {}, platform authority: {}", mint_pubkey, payer.pubkey(), platform_state, platform_authority);


    let csi_project_id = "CSI-BIOCHAR-2026-001".to_string();
    let owner_pubkey = payer.pubkey(); // stand-in for the plant owner's wallet in this test

    let (batch_acc, _batch_bump) = Pubkey::find_program_address(
        &[b"batch", owner_pubkey.as_ref(), csi_project_id.as_bytes()], &program_id);
    

    let create_batch_ix = Instruction::new_with_bytes(
        program_id,
        &ksha_csi_cc::instruction::CreateBatch{
            owner: owner_pubkey,
            csi_project_id,
            verified_amount: 20,
        }.data(),
        ksha_csi_cc::accounts::CreateBatch{
            admin: payer.pubkey(),
            platform_state,
            batch_account: batch_acc,
            system_program: anchor_lang::solana_program::system_program::ID,
        }.to_account_metas(None),
    );

    let owner_ata = get_associated_token_address(&payer.pubkey(), &mint_pubkey);

    // ── build the instruction that actually creates it ──
    let create_ata_ix = create_associated_token_account(
        &payer.pubkey(),        // funds the rent
        &payer.pubkey(),        // wallet that will own this ATA
        &mint_pubkey,            // which mint this ATA holds
        &anchor_spl::token::ID,  // underlying token program
    );

    let mint_batch_ix = Instruction::new_with_bytes(
        program_id,
        &ksha_csi_cc::instruction::MintBatch{
            amount: 20,
        }.data(),
        ksha_csi_cc::accounts::MintBatch{
            payer: payer.pubkey(),
            platform_state,
            platform_authority,
            batch_account: batch_acc,
            mint: mint_pubkey,
            owner_token_account: owner_ata,
            token_program: anchor_spl::token::ID,
        }.to_account_metas(None)
    );
    

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction, create_batch_ix, create_ata_ix, mint_batch_ix], Some(&payer.pubkey()), &blockhash);
    // let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer, mint_keypair]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "transaction failed: {:?}", res);
    println!("Success");

    // Fetch the raw account data litesvm is holding for batch_acc and
    // deserialize it back into your BatchAccount struct to confirm it
    // actually holds what create_batch should have written.
    let batch_account_raw = svm.get_account(&batch_acc).expect("batch account not found");
    let batch_data: ksha_csi_cc::BatchAccount =
        anchor_lang::AccountDeserialize::try_deserialize(&mut batch_account_raw.data.as_slice())
            .expect("failed to deserialize BatchAccount");

    println!(
        "BatchAccount -> owner: {}, csi_project_id: {}, verified_amount: {}, minted_amount: {}, retired_amount: {}, minted: {}",
        batch_data.owner,
        batch_data.csi_project_id,
        batch_data.verified_amount,
        batch_data.minted_amount,
        batch_data.retired_amount,
        batch_data.minted,
    );

    // Actual assertions, not just printing — this is what makes it a real
    // test rather than a script that happens not to crash.
    assert_eq!(batch_data.csi_project_id, "CSI-BIOCHAR-2026-001");
    assert_eq!(batch_data.verified_amount, 20);
    assert_eq!(batch_data.minted_amount, 20);   // mint_batch ran successfully → equals verified_amount
    assert_eq!(batch_data.retired_amount, 0);   // nothing retired yet
    assert_eq!(batch_data.minted, true);        // mint_batch sets this flag

    // Confirms the actual SPL token balance landed correctly — this is
    // the part that proves the real mint happened, not just your
    // program's own bookkeeping flag.
    let ata_account_raw = svm.get_account(&owner_ata).expect("ATA not found");
    let token_account_data = anchor_spl::token::spl_token::state::Account::unpack(&ata_account_raw.data)
        .expect("failed to unpack token account");
    assert_eq!(token_account_data.amount, 20);
    println!("ATA balance: {}", token_account_data.amount);
}


#[test]
fn test_create_batch_rejects_non_admin() {
    let program_id = ksha_csi_cc::id();
    let real_admin = Keypair::new();
    let attacker = Keypair::new(); // not the admin — should be rejected
 
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/ksha_csi_cc.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&real_admin.pubkey(), 1_000_000_000).unwrap();
    svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();
 
    let mint_keypair = Keypair::new();
    let mint_pubkey = Signer::pubkey(&mint_keypair);
 
    let (platform_state, _) = Pubkey::find_program_address(&[b"platform_state"], &program_id);
    let (platform_authority, _) =
        Pubkey::find_program_address(&[b"platform_authority"], &program_id);
 
    // init_platform: real_admin is designated as the one true admin
    let init_ix = Instruction::new_with_bytes(
        program_id,
        &ksha_csi_cc::instruction::InitPlatform {
            admin: real_admin.pubkey(),
        }
        .data(),
        ksha_csi_cc::accounts::InitPlatform {
            payer: real_admin.pubkey(),
            platform_state,
            platform_authority,
            mint: mint_pubkey,
            token_program: anchor_spl::token::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );
 
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[init_ix], Some(&real_admin.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg),
        &[&real_admin, &mint_keypair],
    )
    .unwrap();
    svm.send_transaction(tx).expect("init_platform should succeed");
 
    // ── Now the attacker tries to call create_batch, signing themselves
    // instead of real_admin. This should fail. ──
    let csi_project_id = "FAKE-PROJECT-001".to_string();
    let (batch_acc, _) = Pubkey::find_program_address(
        &[b"batch", attacker.pubkey().as_ref(), csi_project_id.as_bytes()],
        &program_id,
    );
 
    let malicious_create_batch_ix = Instruction::new_with_bytes(
        program_id,
        &ksha_csi_cc::instruction::CreateBatch {
            owner: attacker.pubkey(), // attacker tries to mint to themselves
            csi_project_id,
            verified_amount: 999999, // arbitrary large fake amount
        }
        .data(),
        ksha_csi_cc::accounts::CreateBatch {
            admin: attacker.pubkey(), // attacker signs as themselves, NOT real_admin
            platform_state,
            batch_account: batch_acc,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );
 
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[malicious_create_batch_ix],
        Some(&attacker.pubkey()),
        &blockhash,
    );
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&attacker]).unwrap();
 
    let res = svm.send_transaction(tx);
    assert!(
        res.is_err(),
        "SECURITY BUG: non-admin was able to create a batch! result: {:?}",
        res
    );
    println!("Correctly rejected non-admin create_batch attempt: {:?}", res.err());
}

