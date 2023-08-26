# friendrekt (based on evmcheb's)

Read the [breakdown](https://kg.zaaane.com/mev/friend-tech-sniper) for more
information on this repository. The Rust & Python components need to be running
simultaneously for the bot to work. Thank you to [evmcheb for the thread](https://twitter.com/evmcheb/status/1694614245516955709) and for sharing the [original repo](https://github.com/evmcheb/friendrekt).

## How to run

### Rust component

The Rust project is responsible for parsing transactions and executing
transactions at the right time with the right values.

To run the Rust project (`friendrekt-rs`) you need to have Rust installed. Then
follow these steps:

1. Create a .env file with the following values:

```bash
BASE_WSS_URL=wss://base-mainnet.blastapi.io/{your_secret_id}
FT_ADDRESS=0xCF205808Ed36593aa40a44F10c7f7C2F67d4A4d4 # friend.tech contract address
PRIVATE_KEY=...
SNIPER_ADDRESS=<deploy the smart contracts to get a sniper address>
```

2. Install dependencies and run the project

```bash
cd friendrekt-rs
cargo install --path .
cargo run build.rs # builds abi
cargo run src/main.rs # run mev bot
```

### Python component

The Python project is really simple. All it does is listen for `GET` requests
from the Rust project and returns the number of followers for the specified
Twitter account.

To run the Python project (`friendrekt-py`) you need to have Python 3.10+
installed. Then follow these steps:

1. Create a file at `./friendrekt-py/creds.json` and include your Twitter credentials like so:

```json
{
    "username": "your_twitter_username",
    "password": "your_twitter_password"
}
```

2. Create a virtual environment (optional)

```bash
cd friendrekt-py
virtualenv .
source bin/activate
```

3. Install dependencies and run the HTTP server

```bash
pip3 install -r requirements.txt
python3 -m guvicorn main:app --reload
```

### Contracts

There are also smart contracts included in the project. They have added
features for buying shares compared to the friend.tech contracts. To build
the contracts, you need to have `forge` installed. Then follow these steps:

```bash
cd friendrekt-contracts
forge build
```

To deploy the contracts, use forge or Remix, whichever you prefer.