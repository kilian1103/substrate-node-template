# Decentralised exchange (DEX) implemented in a FRAME pallet

The dex pallet is located in ``pallets/dex/``.

This pallet has the basic functionality to add and remove liquidity into the DEX for two tokens. The corresponding
functions are ``provide_liquidity`` and ``remove_liquidity``.

After adding liquidity one can query the exchange rate of the tokens using ``get_exchange_rate``.

With ``exchange_token`` one can swap a token to the current exchange rate.

Compile the code by ``cargo build -p pallet-dex``

### Things to improve:

- Connect with Polkadot.js frontend
- Add test cases

Authored by Kilian Scheutwinkel
