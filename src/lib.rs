// SPDX-License-Identifier: MIT
//! Stubmark: event tickets that cannot be faked, plus proof you were there.
//!
//! One component is one event. The organizer sets a price, a capacity and three
//! epochs: refunds close, doors open, and the event ends.
//!
//! - `buy` takes exactly the ticket price in TARI and mints a ticket NFT to the buyer.
//! - `refund` burns the ticket and pays the price back, until the refund epoch
//!   (or at any time once the organizer cancels).
//! - `check_in` is called by the organizer or a door-staff key. It marks the
//!   ticket as used once, so a screenshot or copy fails on the second scan, and it
//!   mints a soulbound "I was there" badge to the attendee.
//! - `payout` moves the takings to the organizer only after the event ended and
//!   only if the event was not cancelled.
//!
//! The component has no owner and no upgrade path. Ticket and badge minting are
//! locked to the component, so not even the organizer can mint a free ticket.

use tari_template_lib::prelude::*;

fn push_num(out: &mut String, mut n: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    for b in &buf[i..] {
        out.push(*b as char);
    }
}

#[derive(Debug, Clone, minicbor::Encode, minicbor::Decode, minicbor::CborLen)]
pub struct TicketData {
    #[n(0)]
    pub number: u64,
}

#[derive(Debug, Clone, minicbor::Encode, minicbor::Decode, minicbor::CborLen)]
pub struct BadgeData {
    #[n(0)]
    pub ticket_number: u64,
    #[n(1)]
    pub checked_in_epoch: u64,
}

#[template]
mod stubmark {
    use super::*;

    const MAX_STAFF: usize = 16;

    pub struct Stubmark {
        organizer: RistrettoPublicKeyBytes,
        organizer_account: ComponentAddress,
        title: String,
        price: Amount,
        capacity: u32,
        sold: u32,
        refund_until_epoch: u64,
        doors_open_epoch: u64,
        end_epoch: u64,
        cancelled: bool,
        paid_out: bool,
        staff: Vec<RistrettoPublicKeyBytes>,
        /// Index = ticket number - 1. 0 = refunded, 1 = valid, 2 = used at the door.
        issued: Vec<u8>,
        checked_in: u32,
        ticket_resource: ResourceAddress,
        badge_resource: ResourceAddress,
        takings: Vault,
    }

    impl Stubmark {
        /// Creates an event. The caller becomes the organizer.
        pub fn new(
            organizer_account: ComponentAddress,
            title: String,
            price: Amount,
            capacity: u32,
            refund_until_epoch: u64,
            doors_open_epoch: u64,
            end_epoch: u64,
        ) -> Component<Self> {
            assert!(!title.is_empty(), "title must not be empty");
            assert!(price.is_positive(), "price must be more than zero");
            assert!(capacity > 0, "capacity must be more than zero");
            let now = Consensus::current_epoch();
            assert!(refund_until_epoch > now, "refund window must end in the future");
            assert!(
                doors_open_epoch >= refund_until_epoch,
                "doors cannot open before refunds close"
            );
            assert!(end_epoch > doors_open_epoch, "event must end after doors open");

            let alloc = CallerContext::allocate_component_address(None);
            let me = alloc.get_address();

            // Tickets: only this component may mint or burn them. They are
            // ordinary transferable NFTs, so a ticket can be gifted or resold.
            let ticket_resource = ResourceBuilder::non_fungible()
                .with_token_symbol("TICKET")
                .with_owner_rule(OwnerRule::None)
                .mintable(rule![component(me)], LOCKED)
                .burnable(rule![component(me)], LOCKED)
                .build();

            // Badges: only this component may mint them and nobody can move one
            // out of the vault it was deposited into (soulbound).
            let badge_resource = ResourceBuilder::non_fungible()
                .with_token_symbol("HERE")
                .with_owner_rule(OwnerRule::None)
                .mintable(rule![component(me)], LOCKED)
                .withdrawable(rule![deny_all], LOCKED)
                .build();

            let organizer = CallerContext::transaction_signer_public_key();

            let access_rules = ComponentAccessRules::new()
                .method("buy", rule![allow_all])
                .method("refund", rule![allow_all])
                .method("check_in", rule![allow_all])
                .method("cancel", rule![allow_all])
                .method("payout", rule![allow_all])
                .method("add_staff", rule![allow_all])
                .method("remove_staff", rule![allow_all])
                .method("info", rule![allow_all])
                .method("is_checked_in", rule![allow_all])
                .method("ticket_resource", rule![allow_all])
                .method("badge_resource", rule![allow_all])
                .method("staff_count", rule![allow_all])
                .default(rule![deny_all]);

            emit_event(
                "stubmark.created",
                metadata![
                    "title" => title.clone(),
                    "price" => price,
                    "capacity" => capacity,
                    "refund_until_epoch" => refund_until_epoch,
                    "doors_open_epoch" => doors_open_epoch,
                    "end_epoch" => end_epoch,
                ],
            );

            Component::new(Self {
                organizer,
                organizer_account,
                title,
                price,
                capacity,
                sold: 0,
                refund_until_epoch,
                doors_open_epoch,
                end_epoch,
                cancelled: false,
                paid_out: false,
                staff: Vec::new(),
                issued: Vec::new(),
                checked_in: 0,
                ticket_resource,
                badge_resource,
                takings: Vault::new_empty(TARI_TOKEN),
            })
            .with_address_allocation(alloc)
            .with_access_rules(access_rules)
            .create()
        }

        /// Buys one ticket. The payment must be exactly the price in TARI. The
        /// ticket NFT is deposited into `buyer_account`. Returns the ticket number.
        pub fn buy(&mut self, payment: Bucket, buyer_account: ComponentAddress) -> u64 {
            assert!(!self.cancelled, "event is cancelled");
            assert!(Consensus::current_epoch() < self.end_epoch, "event has ended");
            assert!(self.sold < self.capacity, "sold out");
            assert!(payment.resource_address() == TARI_TOKEN, "payment must be in TARI");
            assert!(payment.amount() == self.price, "payment must equal the ticket price");

            self.takings.deposit(payment);
            self.sold += 1;
            let number = self.sold as u64;
            let id = NonFungibleId::from_u64(number);

            let mut meta = Metadata::new();
            let mut name = self.title.clone();
            name.push_str(" #");
            push_num(&mut name, number);
            meta.insert("name", &name);
            let ticket = ResourceManager::get(self.ticket_resource).mint_non_fungible(
                id.clone(),
                &meta,
                &TicketData { number },
            );
            self.issued.push(1);
            ComponentManager::get(buyer_account).invoke("deposit", args![ticket]);

            emit_event(
                "stubmark.bought",
                metadata!["number" => number, "sold" => self.sold],
            );
            number
        }

        /// Returns a ticket and gets the price back. Allowed until the refund
        /// epoch, or at any time after the organizer cancelled. A used ticket
        /// cannot be refunded.
        pub fn refund(&mut self, ticket: Bucket, refund_account: ComponentAddress) {
            assert!(
                self.cancelled || Consensus::current_epoch() < self.refund_until_epoch,
                "refund window has closed"
            );
            assert!(
                ticket.resource_address() == self.ticket_resource,
                "not a ticket for this event"
            );
            let ids = ticket.get_non_fungible_ids();
            assert!(ids.len() == 1, "refund one ticket at a time");
            let number = ids[0].as_u64().expect("unknown ticket");
            let slot = self.issued.get_mut((number as usize).wrapping_sub(1));
            let slot = slot.expect("unknown ticket");
            assert!(*slot != 0, "unknown ticket");
            assert!(*slot != 2, "ticket was already used");
            *slot = 0;
            ticket.burn();
            let money = self.takings.withdraw(self.price);
            ComponentManager::get(refund_account).invoke("deposit", args![money]);

            emit_event("stubmark.refunded", metadata!["price" => self.price]);
        }

        /// Door check-in by the organizer or a staff key. Marks the ticket used
        /// (a second scan of the same ticket is rejected) and mints a soulbound
        /// badge into `attendee_account`.
        pub fn check_in(&mut self, ticket_number: u64, attendee_account: ComponentAddress) {
            self.assert_door_staff();
            assert!(!self.cancelled, "event is cancelled");
            let now = Consensus::current_epoch();
            assert!(now >= self.doors_open_epoch, "doors are not open yet");
            assert!(now < self.end_epoch, "event has ended");

            let id = NonFungibleId::from_u64(ticket_number);
            let slot = self.issued.get_mut((ticket_number as usize).wrapping_sub(1));
            let slot = slot.expect("unknown or refunded ticket");
            assert!(*slot != 0, "unknown or refunded ticket");
            assert!(*slot != 2, "ticket already used");
            *slot = 2;
            self.checked_in += 1;

            let mut meta = Metadata::new();
            let mut name = String::from("I was at ");
            name.push_str(&self.title);
            meta.insert("name", &name);
            let badge = ResourceManager::get(self.badge_resource).mint_non_fungible(
                id,
                &meta,
                &BadgeData {
                    ticket_number,
                    checked_in_epoch: now,
                },
            );
            ComponentManager::get(attendee_account).invoke("deposit", args![badge]);

            emit_event(
                "stubmark.checked_in",
                metadata!["number" => ticket_number, "checked_in" => self.checked_in],
            );
        }

        /// Cancels the event before doors open. Sales stop, every ticket can
        /// be refunded at any time and the organizer cannot take a payout.
        pub fn cancel(&mut self) {
            self.assert_organizer();
            assert!(!self.cancelled, "already cancelled");
            assert!(
                Consensus::current_epoch() < self.doors_open_epoch,
                "too late to cancel"
            );
            self.cancelled = true;
            emit_event("stubmark.cancelled", metadata!["sold" => self.sold]);
        }

        /// Pays the takings to the organizer. Only after the event ended and
        /// only if it was not cancelled.
        pub fn payout(&mut self) {
            self.assert_organizer();
            assert!(!self.cancelled, "event was cancelled");
            assert!(Consensus::current_epoch() >= self.end_epoch, "event has not ended");
            assert!(!self.paid_out, "already paid out");
            let amount = self.takings.balance();
            assert!(amount.is_positive(), "nothing to pay out");
            let money = self.takings.withdraw(amount);
            ComponentManager::get(self.organizer_account).invoke("deposit", args![money]);
            self.paid_out = true;
            emit_event("stubmark.paid_out", metadata!["amount" => amount]);
        }

        /// Organizer adds a door-staff key that may call `check_in`.
        pub fn add_staff(&mut self, key: RistrettoPublicKeyBytes) {
            self.assert_organizer();
            assert!(self.staff.len() < MAX_STAFF, "too many staff keys");
            assert!(!self.staff.contains(&key), "already staff");
            self.staff.push(key);
        }

        pub fn remove_staff(&mut self, key: RistrettoPublicKeyBytes) {
            self.assert_organizer();
            let before = self.staff.len();
            self.staff.retain(|k| *k != key);
            assert!(self.staff.len() < before, "not a staff key");
        }

        pub fn staff_count(&self) -> u32 {
            self.staff.len() as u32
        }

        /// (title, price, capacity, sold, checked_in, cancelled, paid_out,
        /// refund_until_epoch, doors_open_epoch, end_epoch, takings)
        pub fn info(&self) -> (String, Amount, u32, u32, u32, bool, bool, u64, u64, u64, Amount) {
            (
                self.title.clone(),
                self.price,
                self.capacity,
                self.sold,
                self.checked_in,
                self.cancelled,
                self.paid_out,
                self.refund_until_epoch,
                self.doors_open_epoch,
                self.end_epoch,
                self.takings.balance(),
            )
        }

        pub fn is_checked_in(&self, ticket_number: u64) -> bool {
            self.issued
                .get((ticket_number as usize).wrapping_sub(1))
                .map(|v| *v == 2)
                .unwrap_or(false)
        }

        pub fn ticket_resource(&self) -> ResourceAddress {
            self.ticket_resource
        }

        pub fn badge_resource(&self) -> ResourceAddress {
            self.badge_resource
        }

        fn assert_organizer(&self) {
            assert!(
                CallerContext::transaction_signer_public_key() == self.organizer,
                "only the organizer can do this"
            );
        }

        fn assert_door_staff(&self) {
            let caller = CallerContext::transaction_signer_public_key();
            assert!(
                caller == self.organizer || self.staff.contains(&caller),
                "only the organizer or door staff can check in"
            );
        }
    }
}
