use anchor_client::{
    Client, Cluster, CommitmentConfig, Signer,
};
use solana_sdk::signature::{read_keypair_file, Keypair};
use anchor_lang::prelude::*;
use std::{str::FromStr, sync::Arc};

use askama::Template;
use axum::{
    Router, extract::{Form, Query, State}, response::{Html, IntoResponse}, routing::{get, post},
};
use serde::Deserialize;

declare_program!(ksha_csi_cc);
use ksha_csi_cc::{client::accounts, client::args};

// ── Shared state ──
//
// Earlier attempts used Rc<Keypair>, which failed because Rc is
// explicitly single-threaded (not Sync), and your anchor_client
// version requires signers to satisfy ThreadSafeSigner. Arc<Keypair>
// IS thread-safe, so we can hold the live Program client directly in
// shared state without the per-request-rebuild workaround.
#[derive(Clone)]
struct AppState {
    program: Arc<anchor_client::Program<Arc<Keypair>>>,
    admin: Arc<Keypair>,
    platform_state: Pubkey,
}

#[derive(Template)]
#[template(path = "register_plant.html")]
struct RegisterPlantTemplate {
    success: Option<String>,
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "plant_list.html")]
struct PlantsListTemplate{
    plants: Vec<PlantRow>,
}

struct PlantRow {
    csi_project_id: String,
    owner: String,
}

#[derive(Template)]
#[template(path="create_batch.html")]
struct CreateBatchTemplate{
    project_id: String,
    owner: String,
    success: Option<String>,
    error: Option<String>,
}


#[derive(Deserialize)]
struct RegisterPlantForm {
    csi_project_id: String,
    owner_pubkey: String,
}

#[derive(Deserialize)]
struct CreateBatchQuery {
    project_id: String,
    owner: String,
}

async fn show_form() -> impl IntoResponse {
    let tpl = RegisterPlantTemplate { success: None, error: None };
    Html(tpl.render().unwrap())
}

async fn submit_form(
    State(state): State<AppState>,
    Form(form): Form<RegisterPlantForm>,
) -> impl IntoResponse {
    let owner_pubkey = match Pubkey::from_str(form.owner_pubkey.trim()) {
        Ok(pk) => pk,
        Err(_) => {
            let tpl = RegisterPlantTemplate {
                success: None,
                error: Some(format!(
                    "'{}' doesn't look like a valid Solana wallet address.",
                    form.owner_pubkey
                )),
            };
            return Html(tpl.render().unwrap());
        }
    };

    let csi_project_id = form.csi_project_id.trim().to_string();
    if csi_project_id.is_empty() || csi_project_id.len() > 32 {
        let tpl = RegisterPlantTemplate {
            success: None,
            error: Some("CSI Project ID must be 1-32 characters.".into()),
        };
        return Html(tpl.render().unwrap());
    }

    let (plant_acc, _plant_bump) = Pubkey::find_program_address(
        &[b"plant", csi_project_id.as_bytes()],
        &ksha_csi_cc::ID,
    );

    let mut register_plant_ixs = state
        .program
        .request()
        .accounts(accounts::RegisterPlant {
            admin: state.admin.pubkey(),
            platform_state: state.platform_state,
            plant_account: plant_acc,
            system_program: anchor_lang::solana_program::system_program::ID,
        })
        .args(args::RegisterPlant {
            owner: owner_pubkey,
            csi_project_id: csi_project_id.clone(),
        })
        .instructions();

    if register_plant_ixs.is_empty() {
        let tpl = RegisterPlantTemplate {
            success: None,
            error: Some("Failed to build the registration instruction.".into()),
        };
        return Html(tpl.render().unwrap());
    }
    let register_plant_ix = register_plant_ixs.remove(0);

    let send_result = state
        .program
        .request()
        .instruction(register_plant_ix)
        .signer(state.admin.clone())
        .send()
        .await;

    let tpl = match send_result {
        Ok(signature) => RegisterPlantTemplate {
            success: Some(format!(
                "Registered project {} for {}. Tx: {}",
                csi_project_id, owner_pubkey, signature
            )),
            error: None,
        },
        Err(e) => {
            let msg = e.to_string();
            let friendly = if msg.contains("already in use") {
                format!(
                    "Project {} is already registered. A plant can only be registered once.",
                    csi_project_id
                )
            } else {
                format!("Registration failed: {}", msg)
            };
            RegisterPlantTemplate { success: None, error: Some(friendly) }
        }
    };

    Html(tpl.render().unwrap())
}






// ───────────────────────────────────────────────────────────
// /plants — lists every PlantAccount via getProgramAccounts
// ───────────────────────────────────────────────────────────
// PDAs are deterministic only if you already know the seed (the project ID) — there's no way to derive "every PlantAccount" from
// seed math alone. getProgramAccounts asks the RPC node to scan every account owned by our program and return the ones whose byte layout
// matches PlantAccount's discriminator. This is the standard way to "list all X" for any on-chain account type, given a small/moderate
// account count — at large scale you'd eventually want an indexer instead of scanning on every page load, but that's not a concern at your current plant count.
 
async fn list_plants(State(state): State<AppState>) -> impl IntoResponse {
    let plants = match fetch_all_plants(&state).await {
        Ok(p) => p,
        Err(e) => {
            // Render the list page with zero plants rather than a hard
            // error page — an RPC hiccup shouldn't take the whole admin
            // tool down. The println still surfaces the real cause in
            // your server logs.
            eprintln!("Failed to fetch plants: {}", e);
            vec![]
        }
    };
 
    let tpl = PlantsListTemplate { plants };
    Html(tpl.render().unwrap())
}
 
async fn fetch_all_plants(state: &AppState) -> anyhow::Result<Vec<PlantRow>> {
    // program.accounts::<T>() fetches every account matching T's
    // discriminator automatically — anchor_client handles the
    // discriminator filter for us, we don't construct it by hand.
    let accounts: Vec<(Pubkey, ksha_csi_cc::accounts::PlantAccount)> =
        state.program.accounts(vec![]).await?;
 
    let rows = accounts
        .into_iter()
        .map(|(_pubkey, plant)| PlantRow {
            csi_project_id: plant.csi_project_id,
            owner: plant.owner.to_string(),
        })
        .collect();
 
    Ok(rows)
}



// /show_create_batch_form

async fn show_create_batch_form(
    Query(q): Query<CreateBatchQuery>
) -> impl IntoResponse {
    let tpl = CreateBatchTemplate{
        project_id: q.project_id,
        owner: q.owner,
        success: None,
        error: None
    };
    Html(tpl.render().unwrap())
}




pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/register-plant", get(show_form))
        .route("/register-plant", post(submit_form))
        .route("/plants", get(list_plants))
        .route("/create-batch", get(show_create_batch_form))
        .with_state(state)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let home = std::env::var("HOME")?;
    let admin_path = format!("{}/.config/solana/id.json", home);

    let admin = Arc::new(
        read_keypair_file(&admin_path)
            .map_err(|_| anyhow::anyhow!("Failed to read keypair file at {}", admin_path))?,
    );

    let (platform_state, _state_bump) =
        Pubkey::find_program_address(&[b"platform_state"], &ksha_csi_cc::ID);

    // Client::new_with_options takes the signer directly — pass the
    // SAME Arc<Keypair> we keep in AppState, rather than constructing
    // a second one, so admin's pubkey and the client's payer are
    // guaranteed to be identical.
    let client = Client::new_with_options(
        Cluster::Devnet,
        admin.clone(),
        CommitmentConfig::confirmed(),
    );
    let program = Arc::new(client.program(ksha_csi_cc::ID)?);

    println!("admin: {}, platform_state: {}", admin.pubkey(), platform_state);

    let state = AppState { program, admin, platform_state };

    let app = router(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Listening on http://localhost:3000/register-plant");
    axum::serve(listener, app).await?;

    Ok(())
}








// This is static code and it works through
// #[tokio::main]
// async fn main() -> anyhow::Result<()> {

//     let program_id = ksha_csi_cc::ID;

//     let home = std::env::var("HOME")?;
//     let admin_path = format!("{}/.config/solana/id.json", home);
//     let admin = Arc::new(
//         read_keypair_file(&admin_path)
//             .map_err(|_| anyhow::anyhow!("Failed to read keypair file at {}", admin_path))?
//     );

//     //let plant_owner = Arc::new(Keypair::new()); // This is called here during registration, but in real life this will be input when registering the plant
//     let plant_owner_pubkey = Pubkey::from_str("4dv8abYeJFTK85TT4uSY3G9kHx1ZP2LiohAZ2FYrLS9F")?;
//     // let mint_keypair = Keypair::new();
//     // let mint_pubkey = Signer::pubkey(&mint_keypair); // you only run this once then you can use the pubkey
//     let mint_pubkey = Pubkey::from_str("Gq5TZa5mocsheB18qvGLSmJyygJySsMiGuia5TemVxCr")?;

//     let (platform_state, _state_bump) = Pubkey::find_program_address(&[b"platform_state"], &program_id);
//     let (platform_authority, _authority_bump) = Pubkey::find_program_address(&[b"platform_authority"], &program_id);

//     // Create program client
//     let provider = Client::new_with_options(
//         Cluster::Devnet,
//         Rc::new(admin.clone()),
//         CommitmentConfig::confirmed(),
//     );

//     let program = provider.program(ksha_csi_cc::ID)?;

//     println!("admin: {}, plant owner: {}, mint: {}, platform state: {}, platform authority: {}", admin.pubkey(), plant_owner_pubkey, mint_pubkey, platform_state, platform_authority);

//     // let init_platform_ix = program // Run this only once
//     //             .request()
//     //             .accounts(accounts::InitPlatform{
//     //                 payer: admin.pubkey(),
//     //                 platform_state,
//     //                 platform_authority,
//     //                 mint: mint_pubkey,
//     //                 token_program: anchor_spl::token::ID,
//     //                 system_program: system_program::ID,
//     //             })
//     //             .args(args::InitPlatform{admin: admin.pubkey()})
//     //             .instructions()
//     //             .remove(0);

//     let csi_project_id = "CSI-BIOCHAR-2026-002".to_string();
//     let (plant_acc, _plant_bump) = Pubkey::find_program_address(&[b"plant", csi_project_id.as_bytes()], &program_id);

//     // let register_plant_ix = program
//     //             .request()
//     //             .accounts(accounts::RegisterPlant{
//     //                 admin: admin.pubkey(),
//     //                 platform_state,
//     //                 plant_account: plant_acc,
//     //                 system_program: system_program::ID,
//     //             })
//     //             .args(args::RegisterPlant{
//     //                 owner: plant_owner_pubkey,
//     //                 csi_project_id,
//     //             })
//     //             .instructions()
//     //             .remove(0);
    

//     let (batch_acc, _batch_bump) = Pubkey::find_program_address(
//         &[b"batch", plant_owner_pubkey.as_ref(), csi_project_id.as_bytes()],
//         &program_id,
//     );
//     // let create_batch_ix = program
//     //             .request()
//     //             .accounts(accounts::CreateBatch{
//     //                 admin: admin.pubkey(),
//     //                 platform_state,
//     //                 plant_account: plant_acc,
//     //                 owner: plant_owner_pubkey,
//     //                 batch_account: batch_acc,
//     //                 system_program: system_program::ID,
//     //             })
//     //             .args(args::CreateBatch{
//     //                 csi_project_id,
//     //                 verified_amount: 100
//     //             })
//     //             .instructions()
//     //             .remove(0);
    
//     // let owner_ata = get_associated_token_address(&plant_owner_pubkey, &mint_pubkey);
 
//     // let create_ata_ix = create_associated_token_account(
//     //     &admin.pubkey(),         // admin pays the rent (fee sponsorship)
//     //     &plant_owner_pubkey,   // plant_owner's wallet owns this ATA
//     //     &mint_pubkey,
//     //     &anchor_spl::token::ID,
//     // );

//     // let mint_batch_ix = program
//     //             .request()
//     //             .accounts(accounts::MintBatch{
//     //                 payer: admin.pubkey(),
//     //                 platform_state,
//     //                 platform_authority,
//     //                 batch_account: batch_acc,
//     //                 mint: mint_pubkey,
//     //                 owner_token_account: owner_ata,
//     //                 token_program: anchor_spl::token::ID,
//     //             })
//     //             .args(args::MintBatch{
//     //                 amount: 100
//     //             })
//     //             .instructions()
//     //             .remove(0);


//     // let signature = program
//     //     .request()
//     //     .instruction(create_ata_ix) // init_platform should be only called once, register_plant_ix called only once when registering the plant, create_batch_ix already called but in general this is called with mint
//     //     .instruction(mint_batch_ix)
//     //     .signer(admin.clone())
//     //     //.signer(mint_keypair)
//     //     .send()
//     //     .await?;

//     // println!("Transaction confirmed: {:?}", signature);

//     let app = router();
//     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
//     axum::serve(listener, app).await.unwrap();


//     Ok(())
// }
