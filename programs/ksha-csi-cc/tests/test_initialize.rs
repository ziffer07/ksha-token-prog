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
 
    // ── Two distinct identities, on purpose ──
    // admin: Ksha's operator key. Signs everything. Pays every fee.
    // plant_owner: the real-world plant owner's wallet (their Phantom
    // wallet in production). Generated here so we have a realistic,
    // distinct pubkey — but notice it NEVER appears in any signer list
    // below. It's pure data: "send tokens here," nothing more.
    let admin = Keypair::new();
    let plant_owner = Keypair::new();
 
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/ksha_csi_cc.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&admin.pubkey(), 1_000_000_000).unwrap();
    // Note: plant_owner gets NO airdrop. They don't need SOL — they
    // never pay for or sign anything. This itself is a useful sanity
    // check: if the pipeline only works because plant_owner happens to
    // have SOL, something's wrong with the design.
 
    let mint_keypair = Keypair::new();
    let mint_pubkey = Signer::pubkey(&mint_keypair);
 
    let (platform_state, _state_bump) =
        Pubkey::find_program_address(&[b"platform_state"], &program_id);
    let (platform_authority, _authority_bump) =
        Pubkey::find_program_address(&[b"platform_authority"], &program_id);
 
    // ── init_platform: admin designates itself as the platform admin ──
    let init_ix = Instruction::new_with_bytes(
        program_id,
        &ksha_csi_cc::instruction::InitPlatform { admin: admin.pubkey() }.data(),
        ksha_csi_cc::accounts::InitPlatform {
            payer: admin.pubkey(),
            platform_state,
            platform_authority,
            mint: mint_pubkey,
            token_program: anchor_spl::token::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );
 
    println!(
        "Mint: {}, admin: {}, plant_owner: {}, platform state: {}, platform authority: {}",
        mint_pubkey, admin.pubkey(), plant_owner.pubkey(), platform_state, platform_authority
    );
 
    // ── register_plant: admin signs, plant_owner's pubkey is just data ──
    let csi_project_id = "CSI-BIOCHAR-2026-001".to_string();
    let (plant_acc, _plant_bump) = Pubkey::find_program_address(&[b"plant", csi_project_id.as_bytes()], &program_id);
 
    let register_plant_ix = Instruction::new_with_bytes(
        program_id,
        &ksha_csi_cc::instruction::RegisterPlant {
            owner: plant_owner.pubkey(),
            csi_project_id: csi_project_id.clone(),
        }
        .data(),
        ksha_csi_cc::accounts::RegisterPlant {
            admin: admin.pubkey(),
            platform_state,
            plant_account: plant_acc,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );
 
    // ── create_batch: admin signs, plant_owner appears only as an
    // UncheckedAccount whose KEY is read and compared — not a signer ──
    let (batch_acc, _batch_bump) = Pubkey::find_program_address(
        &[b"batch", plant_owner.pubkey().as_ref(), csi_project_id.as_bytes()],
        &program_id,
    );
 
    let create_batch_ix = Instruction::new_with_bytes(
        program_id,
        &ksha_csi_cc::instruction::CreateBatch {
            csi_project_id: csi_project_id.clone(),
            verified_amount: 20,
        }
        .data(),
        ksha_csi_cc::accounts::CreateBatch {
            admin: admin.pubkey(),
            platform_state,
            plant_account: plant_acc,
            owner: plant_owner.pubkey(), // present as data/key, NOT a signer
            batch_account: batch_acc,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );
 
    // ── ATA: admin funds it, plant_owner's wallet owns it ──
    let owner_ata = get_associated_token_address(&plant_owner.pubkey(), &mint_pubkey);
 
    let create_ata_ix = create_associated_token_account(
        &admin.pubkey(),         // admin pays the rent (fee sponsorship)
        &plant_owner.pubkey(),   // plant_owner's wallet owns this ATA
        &mint_pubkey,
        &anchor_spl::token::ID,
    );
 
    // ── mint_batch: admin signs/pays, tokens land in plant_owner's ATA ──
    let mint_batch_ix = Instruction::new_with_bytes(
        program_id,
        &ksha_csi_cc::instruction::MintBatch { amount: 20 }.data(),
        ksha_csi_cc::accounts::MintBatch {
            payer: admin.pubkey(),
            platform_state,
            platform_authority,
            batch_account: batch_acc,
            mint: mint_pubkey,
            owner_token_account: owner_ata,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
    );
 
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[init_ix, register_plant_ix, create_batch_ix, create_ata_ix, mint_batch_ix],
        Some(&admin.pubkey()),
        &blockhash,
    );
 
    // ── Only admin and mint_keypair sign. plant_owner is NOT here. ──
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&admin, &mint_keypair])
        .unwrap();
 
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "transaction failed: {:?}", res);
    println!("Success — plant_owner never signed anything, only received tokens.");
 
    let batch_account_raw = svm.get_account(&batch_acc).expect("batch account not found");
    let batch_data: ksha_csi_cc::BatchAccount =
        anchor_lang::AccountDeserialize::try_deserialize(&mut batch_account_raw.data.as_slice())
            .expect("failed to deserialize BatchAccount");
 
    assert_eq!(batch_data.owner, plant_owner.pubkey()); // confirms ownership was read from PlantAccount, not faked
    assert_eq!(batch_data.csi_project_id, csi_project_id);
    assert_eq!(batch_data.verified_amount, 20);
    assert_eq!(batch_data.minted_amount, 20);
    assert_eq!(batch_data.minted, true);
 
    let ata_account_raw = svm.get_account(&owner_ata).expect("ATA not found");
    let token_account_data =
        anchor_spl::token::spl_token::state::Account::unpack(&ata_account_raw.data)
            .expect("failed to unpack token account");
    assert_eq!(token_account_data.amount, 20);
    println!("plant_owner's ATA balance: {}", token_account_data.amount);
}






#[test]
fn test_create_batch_rejects_non_admin() {
    let program_id = ksha_csi_cc::id();
    let real_admin = Keypair::new();
    let plant_owner = Keypair::new();
    let attacker = Keypair::new(); // not the admin — should be rejected
 
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/ksha_csi_cc.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&real_admin.pubkey(), 1_000_000_000).unwrap();
    svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();
 
    let mint_keypair = Keypair::new();
    let mint_pubkey = Signer::pubkey(&mint_keypair);
 
    let (platform_state, _) = Pubkey::find_program_address(&[b"platform_state"], &program_id);
    let (platform_authority, _) = Pubkey::find_program_address(&[b"platform_authority"], &program_id);
    
 
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

    // ── register_plant: admin signs, plant_owner's pubkey is just data ──
    let csi_project_id = "FAKE-PROJECT-001".to_string();
    let (plant_acc, _plant_bump) = Pubkey::find_program_address(&[b"plant", csi_project_id.as_bytes()], &program_id);
 
    let register_plant_ix = Instruction::new_with_bytes(
        program_id,
        &ksha_csi_cc::instruction::RegisterPlant {
            owner: plant_owner.pubkey(),
            csi_project_id: csi_project_id.clone(),
        }
        .data(),
        ksha_csi_cc::accounts::RegisterPlant {
            admin: real_admin.pubkey(),
            platform_state,
            plant_account: plant_acc,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );
 
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[init_ix, register_plant_ix], Some(&real_admin.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(
        VersionedMessage::Legacy(msg),
        &[&real_admin, &mint_keypair],
    )
    .unwrap();
    svm.send_transaction(tx).expect("init_platform should succeed");
 
    // ── Now the attacker tries to call create_batch, signing themselves
    // instead of real_admin. This should fail. ──
    let (batch_acc, _) = Pubkey::find_program_address(
        &[b"batch", attacker.pubkey().as_ref(), csi_project_id.as_bytes()],
        &program_id,
    );
 
    let malicious_create_batch_ix = Instruction::new_with_bytes(
        program_id,
        &ksha_csi_cc::instruction::CreateBatch {
            csi_project_id,
            verified_amount: 999999, // arbitrary large fake amount
        }
        .data(),
        ksha_csi_cc::accounts::CreateBatch {
            admin: attacker.pubkey(),
            platform_state,
            plant_account: plant_acc,
            owner: plant_owner.pubkey(), // present as data/key, NOT a signer
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

