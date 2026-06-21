# Ksha

Tokenizing Real World Assets (RWAs) on Solana.

## Current Status

This repository contains the first version of the Ksha landing page built with:

* Rust
* Axum
* Askama templates
* HTML/CSS

The current site explains:

* What Ksha does
* How the verification → tokenization flow works
* Why faster settlement matters for producers
* How users can request a demo

## Running Locally

### Prerequisites

* Rust (latest stable)

### Start the server

```bash
cargo run
```

The application will be available at:

```text
http://127.0.0.1:3000
```

## Project Structure

```text
.
├── src/
│   └── main.rs
├── templates/
│   └── home.html
├── Cargo.toml
└── README.md
```

## Roadmap

Planned features include:

* Demo booking flow
* Authentication
* Verification integrations
* Tokenization dashboard
* Wallet support
* Settlement tracking
* Producer onboarding
* Marketplace

## Vision

Ksha aims to become the infrastructure layer connecting verified assets with faster, more transparent financial settlement.

## License

Private and proprietary. All rights reserved.
