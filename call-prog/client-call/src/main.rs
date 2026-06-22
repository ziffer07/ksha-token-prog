use anchor_client::{Client, Cluster, CommitmentConfig, Signer};
use anchor_lang::prelude::*;
use anchor_spl::associated_token::{get_associated_token_address, spl_associated_token_account::instruction::create_associated_token_account};
use solana_sdk::{signature::{read_keypair_file, Keypair}};
use std::{rc::Rc, str::FromStr, sync::Arc};


declare_program!(ksha_csi_cc);
use ksha_csi_cc::{client::accounts, client::args};


#[tokio::main]
async fn main() -> anyhow::Result<()> {

    let program_id = ksha_csi_cc::ID;

    let home = std::env::var("HOME")?;
    let admin_path = format!("{}/.config/solana/id.json", home);
    let admin = Arc::new(
        read_keypair_file(&admin_path)
            .map_err(|_| anyhow::anyhow!("Failed to read keypair file at {}", admin_path))?
    );

    //let plant_owner = Arc::new(Keypair::new()); // This is called here during registration, but in real life this will be input when registering the plant
    let plant_owner_pubkey = Pubkey::from_str("4dv8abYeJFTK85TT4uSY3G9kHx1ZP2LiohAZ2FYrLS9F")?;
    // let mint_keypair = Keypair::new();
    // let mint_pubkey = Signer::pubkey(&mint_keypair); // you only run this once then you can use the pubkey
    let mint_pubkey = Pubkey::from_str("Gq5TZa5mocsheB18qvGLSmJyygJySsMiGuia5TemVxCr")?;

    let (platform_state, _state_bump) = Pubkey::find_program_address(&[b"platform_state"], &program_id);
    let (platform_authority, _authority_bump) = Pubkey::find_program_address(&[b"platform_authority"], &program_id);

    // Create program client
    let provider = Client::new_with_options(
        Cluster::Devnet,
        Rc::new(admin.clone()),
        CommitmentConfig::confirmed(),
    );

    let program = provider.program(ksha_csi_cc::ID)?;

    println!("admin: {}, plant owner: {}, mint: {}, platform state: {}, platform authority: {}", admin.pubkey(), plant_owner_pubkey, mint_pubkey, platform_state, platform_authority);

    // let init_platform_ix = program // Run this only once
    //             .request()
    //             .accounts(accounts::InitPlatform{
    //                 payer: admin.pubkey(),
    //                 platform_state,
    //                 platform_authority,
    //                 mint: mint_pubkey,
    //                 token_program: anchor_spl::token::ID,
    //                 system_program: system_program::ID,
    //             })
    //             .args(args::InitPlatform{admin: admin.pubkey()})
    //             .instructions()
    //             .remove(0);

    let csi_project_id = "CSI-BIOCHAR-2026-002".to_string();
    let (plant_acc, _plant_bump) = Pubkey::find_program_address(&[b"plant", csi_project_id.as_bytes()], &program_id);

    // let register_plant_ix = program
    //             .request()
    //             .accounts(accounts::RegisterPlant{
    //                 admin: admin.pubkey(),
    //                 platform_state,
    //                 plant_account: plant_acc,
    //                 system_program: system_program::ID,
    //             })
    //             .args(args::RegisterPlant{
    //                 owner: plant_owner_pubkey,
    //                 csi_project_id,
    //             })
    //             .instructions()
    //             .remove(0);
    

    let (batch_acc, _batch_bump) = Pubkey::find_program_address(
        &[b"batch", plant_owner_pubkey.as_ref(), csi_project_id.as_bytes()],
        &program_id,
    );
    let create_batch_ix = program
                .request()
                .accounts(accounts::CreateBatch{
                    admin: admin.pubkey(),
                    platform_state,
                    plant_account: plant_acc,
                    owner: plant_owner_pubkey,
                    batch_account: batch_acc,
                    system_program: system_program::ID,
                })
                .args(args::CreateBatch{
                    csi_project_id,
                    verified_amount: 100
                })
                .instructions()
                .remove(0);
    
    let owner_ata = get_associated_token_address(&plant_owner_pubkey, &mint_pubkey);
 
    let create_ata_ix = create_associated_token_account(
        &admin.pubkey(),         // admin pays the rent (fee sponsorship)
        &plant_owner_pubkey,   // plant_owner's wallet owns this ATA
        &mint_pubkey,
        &anchor_spl::token::ID,
    );

    let mint_batch_ix = program
                .request()
                .accounts(accounts::MintBatch{
                    payer: admin.pubkey(),
                    platform_state,
                    platform_authority,
                    batch_account: batch_acc,
                    mint: mint_pubkey,
                    owner_token_account: owner_ata,
                    token_program: anchor_spl::token::ID,
                })
                .args(args::MintBatch{
                    amount: 100
                })
                .instructions()
                .remove(0);


    let signature = program
        .request()
        .instruction(create_ata_ix) // init_platform should be only called once, register_plant_ix called only once when registering the plant, create_batch_ix already called but in general this is called with mint
        .instruction(mint_batch_ix)
        .signer(admin.clone())
        //.signer(mint_keypair)
        .send()
        .await?;

    println!("Transaction confirmed: {:?}", signature);

    Ok(())
}
