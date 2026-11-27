# Stubmark

Event tickets on Tari Ootle that cannot be faked, with refunds, door check-in and a soulbound proof-of-attendance badge. MIT licensed.

One component is one event. The organizer sets title, price, capacity, a refund deadline, a doors-open epoch and an end epoch. All money sits in the component's vault, so guests do not have to trust a seller.

## What it does

| Step | Rule enforced by the template |
|---|---|
| `new` | Non-empty title, positive price, capacity above zero, refund deadline in the future, doors not before the deadline, end after doors |
| `buy` | Exact TARI price only, not cancelled, not ended, not sold out. Mints one ticket NFT to the buyer |
| `refund` | Before the deadline (any time if cancelled). Burns the ticket, returns the price. Used or foreign tickets are rejected |
| `check_in` | Organizer or listed door staff only, only while doors are open, one time per ticket. Mints a soulbound badge to the attendee |
| `cancel` | Organizer only, before doors. Opens refunds forever and blocks payout |
| `payout` | After the end, once, to the organizer, never if cancelled |
| `add_staff` / `remove_staff` | Organizer only, up to 16 door keys |

Tickets can only be minted by the component (address allocation lock) and are burnable only by it. Tickets are ordinary transferable NFTs, so gifting works and the new holder can refund. Badges are soulbound: they cannot be withdrawn from the account.

## Tests

```
cargo test
```

28 engine tests run the real template in `tari_template_test_tooling`, moving the epoch by hand. A full run takes about 90 seconds on 2 cores. Covered: price and capacity limits, every refund path, cancel and payout rules, staff management and the staff cap, check-in window, stranger rejection, double scan rejection, unknown and refunded ticket rejection, badge cannot move, only the component mints, and the invariant takings = sales - refunds.

## Demo

`demo/index.html` is a single static page, no wallet, no server. It mirrors the template rules in JavaScript so anyone can try buy, refund, cancel, payout and the gate with a rotating QR code. Open the file or serve the folder.

## Honest limits

- The rotating QR code (new code every 5 seconds) is an off-chain front-end layer. It stops screenshots. The on-chain protection is the used flag: a second check-in is rejected by the template.
- The demo simulates the template. The Rust tests are the proof of the real behavior.
- `sold` counts tickets ever issued, so a refund does not free a seat.
- Binary is built with `opt-level = z`, LTO and `strip`; size is about 200 KiB after the wallet's wasm-opt pass, above the 96 KiB fee allowance, so publishing costs roughly 2 TARI (see the Ootle fee reference).
- Status on Esmeralda: see the bottom of this file.

## Build

```
cargo build --release --target wasm32-unknown-unknown
```

## License

MIT, see `LICENSE`.

## Links

- Website: https://stubmark.pages.dev/
- Live demo (runs the template rules in your browser, no wallet): https://stubmark.pages.dev/demo/
- Demo video: https://youtu.be/cFXifmyB5Rs
- Source: https://github.com/dannytungadi5/stubmark (MIT)

Brand: a ticket stub with a check mark. Ink #0B1020, lime #C8F751, coral #FF6B4A, paper #F4EFE6.

## Esmeralda status

Published on the Esmeralda testnet on 27 Nov 2026 (throwaway testnet wallet, no value).

- Template address: `template_f1dbdb43e285d7b8c91a79b6f350d674ef5bce3a30e55cc86d5d4c0af2af9815`
- Publish tx (Accepted, fee 2.29 tTARI, 205,934 bytes after wasm-opt): `a4daa677c64833430ccb48e9f2da7a087e68da4b7c3acedccec77bed71f3ab12`
- Event component created: `component_22273851cf736d909eb0ed9616a36e32beab2007e52f5dcdfd9f16e191c7b007` (tx `9015dff62690355cc87f608d94293e2d7d4044e6b74841954af14d673ae68994`)
- Buy ticket 1: `ad5b502dac7f25e6c44fe53ef430209793593f6732f912bc55908ee4825ac8ff`
- Buy ticket 2: `ba1b965e14a88b9b6fe793c91ec56cf8b76f7b75aa19463b374b89ee53dd0ec9`
- Refund ticket 1: `767380777d4f0867240952816cbd30835c22fc35fbc8ece094ecf22041a836a8`
- Check-in ticket 2 (doors open, soulbound badge minted): `7ec7c6056937bf95c942901789a5c122571aaeac6986fdc2c3e669a9421ce646`
- Second full cycle (new event `component_4fbc7f3e08ce0176236ac47d99840c9957794a1e9aabc4eb6e5aa8414313c74c`, refund window and doors both at epoch 13442):
  - Create: `f705b5afe8c4e56e8f19973f0fd69ce037c74d2f8054bb769b1b361cad6f1599`
  - Buy ticket 1: `cd40df51109cc1edfe866d3f21bf4680804b319edbb3df51898612d0b0a1fb66`
  - Buy ticket 2: `9cb9e56e09de7a21915338fd07339244b5850df0f917d84a4e333ef2ee64b2f1`
  - Refund ticket 1 (before the cutoff): `a0e9bab6bd650bbf027878d460a03e274d84259f0313af97eba74347f4bca7c1`
  - Check-in ticket 2 (doors open, soulbound badge minted): `ff45f96c21edaafe59c762f0de92e66a926a94b20762aa66ff2cdb3765842824`

Verified in the Ootle test engine too (28 tests).
