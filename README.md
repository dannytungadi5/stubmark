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
- Binary is built with `opt-level = z`, LTO and `strip`; size before the wallet's wasm-opt pass is about 248 KiB, above the 96 KiB fee allowance. Publishing costs roughly 3 to 4 TARI at that size (see the Ootle fee reference).
- Status on Esmeralda: see the bottom of this file.

## Build

```
cargo build --release --target wasm32-unknown-unknown
```

## License

MIT, see `LICENSE`.

## Esmeralda status

Not published on Esmeralda yet. On 27 Nov the public faucet held about 1.7 tXTR and allowed 0 per withdrawal, and publishing a template of this size costs about 3 to 4 tXTR, so there was nothing to pay the fee with. The template is verified in the Ootle test engine only (28 tests). If funds appear, publish with the wallet daemon's Publish Template page.
