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
- Status on Esmeralda: see "Verify on chain" below.

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

## Verify on chain

Everything below ran on the Esmeralda testnet (throwaway wallet, no value). Click a transaction to see it in the Veil explorer. Each one should show as accepted on network `esmeralda`.

- Template address: `template_f1dbdb43e285d7b8c91a79b6f350d674ef5bce3a30e55cc86d5d4c0af2af9815`
- Cycle 1 event component: `component_22273851cf736d909eb0ed9616a36e32beab2007e52f5dcdfd9f16e191c7b007`
- Cycle 2 event component: `component_4fbc7f3e08ce0176236ac47d99840c9957794a1e9aabc4eb6e5aa8414313c74c`

| Step | Transaction |
|---|---|
| Publish template (fee 2.29 tTARI) | [`a4daa677c648...`](https://testnet-explorer.tari.mw/tx/a4daa677c64833430ccb48e9f2da7a087e68da4b7c3acedccec77bed71f3ab12) |
| Cycle 1: create event | [`9015dff62690...`](https://testnet-explorer.tari.mw/tx/9015dff62690355cc87f608d94293e2d7d4044e6b74841954af14d673ae68994) |
| Cycle 1: buy ticket 1 | [`ad5b502dac7f...`](https://testnet-explorer.tari.mw/tx/ad5b502dac7f25e6c44fe53ef430209793593f6732f912bc55908ee4825ac8ff) |
| Cycle 1: buy ticket 2 | [`ba1b965e14a8...`](https://testnet-explorer.tari.mw/tx/ba1b965e14a88b9b6fe793c91ec56cf8b76f7b75aa19463b374b89ee53dd0ec9) |
| Cycle 1: refund ticket 1 | [`767380777d4f...`](https://testnet-explorer.tari.mw/tx/767380777d4f0867240952816cbd30835c22fc35fbc8ece094ecf22041a836a8) |
| Cycle 1: check-in ticket 2 (badge minted) | [`7ec7c6056937...`](https://testnet-explorer.tari.mw/tx/7ec7c6056937bf95c942901789a5c122571aaeac6986fdc2c3e669a9421ce646) |
| Cycle 2: create event | [`f705b5afe8c4...`](https://testnet-explorer.tari.mw/tx/f705b5afe8c4e56e8f19973f0fd69ce037c74d2f8054bb769b1b361cad6f1599) |
| Cycle 2: buy ticket 1 | [`cd40df51109c...`](https://testnet-explorer.tari.mw/tx/cd40df51109cc1edfe866d3f21bf4680804b319edbb3df51898612d0b0a1fb66) |
| Cycle 2: buy ticket 2 | [`9cb9e56e09de...`](https://testnet-explorer.tari.mw/tx/9cb9e56e09de7a21915338fd07339244b5850df0f917d84a4e333ef2ee64b2f1) |
| Cycle 2: refund ticket 1 (before cutoff) | [`a0e9bab6bd65...`](https://testnet-explorer.tari.mw/tx/a0e9bab6bd650bbf027878d460a03e274d84259f0313af97eba74347f4bca7c1) |
| Cycle 2: check-in ticket 2 (badge minted) | [`ff45f96c21ed...`](https://testnet-explorer.tari.mw/tx/ff45f96c21edaafe59c762f0de92e66a926a94b20762aa66ff2cdb3765842824) |

### Rejected on purpose

These three transactions were sent on cycle 2 to show the rules hold on chain. The template rejects each one, and the network charges only the fee (status `OnlyFeeAccepted`, about 0.002 tTARI each). The wallet returned the reason shown.

| Attempt | Transaction | Result |
|---|---|---|
| Second check-in of the same ticket | [`71f10d49b97a...`](https://testnet-explorer.tari.mw/tx/71f10d49b97a739da3622e9c910a4de1cf5411b248c44911c6d4610b42d21bda) | rejected: "ticket already used" |
| Refund of a used ticket after the cutoff | [`3f581998f96c...`](https://testnet-explorer.tari.mw/tx/3f581998f96c38b885fe362d6e40674998099559bf51fc81713b7ffa3080fb3f) | rejected: "refund window has closed" |
| Withdraw the soulbound badge | [`0e1d508b6908...`](https://testnet-explorer.tari.mw/tx/0e1d508b6908d3984c7525c1dd9d1b5022a2825048da6c998c73b04902a83f5d) | rejected: Access Denied on resource Withdraw |

Explorer: https://testnet-explorer.tari.mw/ . The engine tests (`cargo test`) cover the same rules.

## Deploy your own

1. Install Rust and the wasm target: `rustup target add wasm32-unknown-unknown`.
2. Build: `cargo build --release --target wasm32-unknown-unknown`. The file is `target/wasm32-unknown-unknown/release/*.wasm`.
3. Run `cargo test` to check the rules locally (about 90 seconds).
4. Get an Ootle wallet on Esmeralda and fund it with testnet tTARI.
5. Publish the wasm from the wallet (about 2 tTARI at this size, see Honest limits).
6. Call `new` with a title, price, capacity, refund deadline epoch, doors-open epoch and end epoch. The result is your event component.
7. Share the component address. Guests call `buy`, `refund`. Door staff call `check_in`.

The browser demo in `demo/index.html` needs no wallet and no build.
