# Ksha - Plant Onboarding Admin Tool & Landing Page

Internal documentation for the two pieces built so far: the public-facing landing page (for plant owner outreach) and the admin tool for registering plants on-chain.

## Admin Onboarding Tool (Rust + Axum + Askama)

A small internal server, not a public product. Lets Ksha's admin register a plant owner's wallet against a known project ID - the only manual step in an otherwise API-driven pipeline.

### Why this exists

The on-chain program enforces that `create_batch` can only mint against a project whose owner wallet is already registered (`PlantAccount`). This tool is the one-time interface for doing that registration, run personally by the admin during onboarding. Plant owners never sign or pay for anything themselves.

### Structure

```
ksha-admin-form/
├── templates/
│   └── register_plant.html   # Askama template - the actual form UI
└── main.rs                    # Axum server + on-chain call logic
```

### Running it

```bash
cargo run
# visit http://localhost:3000/register-plant
```

Requires:
- A funded admin keypair at `~/.config/solana/id.json` (devnet SOL for fees/rent).
- The Ksha program already deployed, with `init_platform` already run once on that cluster.
- `Cargo.toml` dependencies: `anchor-client`, `anchor-lang`, `askama`, `axum`, `serde`, `anyhow`.

### What the form does

Two fields only:

| Field | What it does |
|---|---|
| Project ID | e.g. `GCSP1084` - must match registry exactly |
| Plant Owner Wallet Address | the plant owner's Solana wallet - confirm this with them directly before submitting |

On submit, the server:
1. Validates the wallet string actually parses as a `Pubkey` (bad paste → error banner, not a crash).
2. Derives the `plant_account` PDA from the project ID.
3. Builds and sends the `register_plant` instruction, signed by the admin keypair only.
4. Shows a success banner with the transaction signature, or a clear error - including a specific message if the project was already registered (a plant can only be registered once; this is enforced on-chain via Anchor's `init` constraint, not just in this tool).

### Known open items

- **Mint address**: `mint_batch`/ATA-related calls need the real mint pubkey, currently best fetched live from `platform_state.mint` rather than hardcoded, since a hardcoded address silently breaks if the program is ever redeployed fresh.
- **Crate version sensitivity**: `anchor_client`'s signer types (`Arc` vs `Rc`, `ThreadSafeSigner`) have shifted across recent versions - if `cargo build` errors on signer trait bounds after a dependency update, check `anchor_client`'s current re-export paths before assuming the code is wrong.
- **This tool is intentionally not exposed publicly.** No auth is implemented because it's assumed to run locally, accessible only to the admin. Do not deploy this to a public-facing host without adding real authentication first.

---

## Status

Both pieces are functional for their intended scope - landing page for outreach, admin tool for manual onboarding. Next steps from here, per earlier planning: wire the API polling loop so `create_batch`/`mint_batch` run automatically once a registered project shows a new verified batch, rather than requiring manual triggering.

---

## TODOs

Update the /plants route. Here, I should only show the tokens that are minted and an input bar where the buyer places order for the amount of tokens they need at their price or the market price (either). Then write logic to make that transfer happen. Should that process be abstracted? Maybe?
Add a login option where only authorized buyers are allowed to participate in the market. Once the KYC is done they can participate.