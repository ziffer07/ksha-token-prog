# KSHA CSI Token Program

A Solana smart contract built with Anchor that tokenizes verified CSI into fungible SPL tokens.

The program maintains a **single global mint** and allows verified CSI batches to be:

1. Registered on-chain
2. Minted into csi tokens
3. Retired (burned) when used to offset emissions

---

## Overview

This MVP implements a simple csi lifecycle:

```text
Verified csi Batch
        │
        ▼
 Create Batch
        │
        ▼
 Mint csi Tokens
        │
        ▼
 Hold / Transfer Tokens
        │
        ▼
 Retire (Burn) Tokens
```

Each verified csi batch represents a quantity of certified csi offsets.

Once verified, a batch can be tokenized into SPL tokens using a single platform-wide mint.

---

## Features

### Platform Initialization

Creates:

* Global csi mint
* Platform state account
* Program-derived mint authority (PDA)

The PDA becomes the mint authority and is controlled exclusively by the program.

### Batch Creation

Creates an immutable on-chain record containing:

* Batch owner
* CSI project ID
* Verified csi amount
* Minting status
* Retirement tracking

### CSI Minting

Allows minting CSI tokens:

* 1 token = 1 verified csi
* Can only mint once per batch
* Mint amount must equal verified amount
* Uses PDA signing for mint authority

### CSI Retirement

Allows holders to permanently retire csi by burning tokens.

Retirement history is tracked inside the batch account.

---

## Program ID

```rust
declare_id!("3J9w5Aof5M4CZtWCmQpEVAQTT1Z26AViYR8mxMnwvdq6");
```

---

# Architecture

## Accounts

### PlatformState

Global singleton storing platform configuration.

```rust
pub struct PlatformState {
    pub mint: Pubkey,
    pub authority_bump: u8,
    pub bump: u8,
}
```

Stores:

* Global mint address
* Platform authority PDA bump
* Platform state PDA bump

---

### BatchAccount

Represents a verified csi batch.

```rust
pub struct BatchAccount {
    pub owner: Pubkey,
    pub csi_project_id: String,
    pub verified_amount: u64,
    pub minted_amount: u64,
    pub retired_amount: u64,
    pub minted: bool,
    pub bump: u8,
}
```

Fields:

| Field           | Description                 |
| --------------- | --------------------------- |
| owner           | Batch owner                 |
| csi_project_id  | CSI verification identifier |
| verified_amount | Total verified credits      |
| minted_amount   | Total minted tokens         |
| retired_amount  | Total retired tokens        |
| minted          | Prevents double minting     |
| bump            | PDA bump                    |

---

# PDA Design

## Platform State PDA

```text
Seed:
["platform_state"]
```

Stores platform configuration.

---

## Platform Authority PDA

```text
Seed:
["platform_authority"]
```

Acts as the mint authority.

Only the program can sign for this PDA.

---

## Batch PDA

```text
Seed:
[
    "batch",
    owner_pubkey,
    csi_project_id
]
```

Ensures every batch is uniquely identifiable.

---

# Instructions

## 1. init_platform

Initializes the platform.

### Creates

* PlatformState PDA
* Platform Authority PDA
* Global SPL Mint

### Result

The mint authority is assigned to the Platform Authority PDA.

---

## 2. create_batch

Creates a verified csi batch.

### Parameters

```rust
csi_project_id: String
verified_amount: u64
```

### Validation

* Project ID ≤ 32 characters
* Verified amount > 0

### Result

Creates a new BatchAccount.

---

## 3. mint_batch

Mints csi tokens.

### Parameters

```rust
amount: u64
```

### Validation

* Batch has not already been minted
* Amount equals verified amount
* Mint matches platform mint

### Result

```text
verified_amount = minted_amount
```

Tokens are minted into the owner's token account.

---

## 4. retire_batch

Burns csi tokens.

### Parameters

```rust
amount: u64
```

### Validation

```text
retired_amount + amount <= minted_amount
```

### Result

Tokens are permanently destroyed.

Retirement amount is recorded on-chain.

---

# Minting Flow

```text
Admin
 │
 ▼
init_platform
 │
 ▼
Global csi Mint
 │
 ▼
Create Batch
 │
 ▼
Verified Batch Account
 │
 ▼
mint_batch()
 │
 ▼
Owner Token Account
```

---

# Retirement Flow

```text
Holder
 │
 ▼
retire_batch()
 │
 ▼
SPL Burn
 │
 ▼
retired_amount += amount
```

---

# Security Model

### Double-Mint Protection

Each batch can only be minted once.

```rust
require!(!batch.minted, ErrorCode::AlreadyMinted);
```

---

### Mint Integrity

Only the platform mint may be used.

```rust
require_keys_eq!(
    ctx.accounts.mint.key(),
    ctx.accounts.platform_state.mint,
    ErrorCode::MintMismatch
);
```

---

### Retirement Protection

Cannot retire more credits than have been minted.

```rust
retired_amount <= minted_amount
```

---

### PDA-Controlled Mint Authority

The mint authority is a PDA.

No private key exists.

Only program logic can authorize minting.

---

# Error Codes

| Error               | Description                              |
| ------------------- | ---------------------------------------- |
| ProjectIdTooLong    | Project ID exceeds 32 chars              |
| ZeroAmount          | Amount must be greater than zero         |
| AlreadyMinted       | Batch already minted                     |
| AmountMismatch      | Mint amount differs from verified amount |
| MintMismatch        | Invalid mint account                     |
| RetireExceedsMinted | Retirement exceeds supply                |

---

# Future Improvements

### Onboarding

* Plant registration
* Project registration
* Verification workflow

### Governance

* DAO-controlled mint approvals
* Multi-signature verification

### Metadata

* Token metadata integration
* Project metadata storage

### Retirement Certificates

* On-chain retirement receipts
* NFT retirement certificates

### Marketplace

* Csi trading
* Credit exchange pools

---

# Build

```bash
anchor build
```

---

# Test

```bash
anchor test
```

or

```bash
cargo test
```

---

# License

MIT License

---

## Disclaimer

This implementation is an MVP intended for educational and prototyping purposes. Production deployments should undergo security review, formal auditing, and additional governance controls before managing real-world csi assets.
