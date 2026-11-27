// SPDX-License-Identifier: MIT
//! Engine tests for Stubmark. Each test runs the real template in the Ootle
//! test engine: money moves, access rules run, epochs are moved by hand.
use tari_template_lib::{
    prelude::{Amount, ComponentAddress, ResourceAddress, RistrettoPublicKeyBytes, TARI_TOKEN},
    types::NonFungibleAddress,
};
use tari_template_test_tooling::{
    TemplateTest,
    crypto::{RistrettoPublicKey, RistrettoSecretKey},
    engine_types::virtual_substate::{VirtualSubstate, VirtualSubstateId},
    support::assert_error::assert_reject_reason,
    transaction::args,
};
use tari_utilities::byte_array::ByteArray;
use tari_template_test_tooling::crypto::PublicKey as _;

const TEMPLATE: &str = "Stubmark";
const PRICE: u64 = 50_000;
const CAPACITY: u32 = 3;
const REFUND_UNTIL: u64 = 100;
const DOORS: u64 = 200;
const END: u64 = 300;

struct Actor {
    account: ComponentAddress,
    proof: NonFungibleAddress,
    key: RistrettoSecretKey,
}

struct Event {
    component: ComponentAddress,
    tickets: ResourceAddress,
    badges: ResourceAddress,
}

fn actor(test: &mut TemplateTest) -> Actor {
    let (account, proof, key) = test.create_funded_account();
    Actor { account, proof, key }
}

fn pubkey(key: &RistrettoSecretKey) -> RistrettoPublicKeyBytes {
    let pk = RistrettoPublicKey::from_secret_key(key);
    RistrettoPublicKeyBytes::from_bytes(pk.as_bytes()).unwrap()
}

fn set_epoch(test: &mut TemplateTest, epoch: u64) {
    test.set_virtual_substate(VirtualSubstateId::CurrentEpoch, VirtualSubstate::CurrentEpoch(epoch));
}

fn setup() -> (TemplateTest, Actor, Event) {
    let mut test = TemplateTest::my_crate();
    set_epoch(&mut test, 1);
    let organizer = actor(&mut test);
    let event = create_event(&mut test, &organizer, "Jazz Night", PRICE, CAPACITY, REFUND_UNTIL, DOORS, END);
    (test, organizer, event)
}

#[allow(clippy::too_many_arguments)]
fn try_create(
    test: &mut TemplateTest,
    organizer: &Actor,
    title: &str,
    price: u64,
    capacity: u32,
    refund_until: u64,
    doors: u64,
    end: u64,
) -> Result<(), String> {
    let template = test.get_template_address(TEMPLATE);
    let tx = test
        .transaction()
        .call_function(
            template,
            "new",
            args![
                organizer.account,
                title.to_string(),
                Amount::from(price),
                capacity,
                refund_until,
                doors,
                end
            ],
        )
        .build_and_seal(&organizer.key);
    match test.try_execute(tx, vec![organizer.proof.clone()]) {
        Ok(r) => match r.finalize.result.is_accept() {
            true => Ok(()),
            false => Err("rejected".to_string()),
        },
        Err(e) => Err(format!("{e:?}")),
    }
}

#[allow(clippy::too_many_arguments)]
fn create_event(
    test: &mut TemplateTest,
    organizer: &Actor,
    title: &str,
    price: u64,
    capacity: u32,
    refund_until: u64,
    doors: u64,
    end: u64,
) -> Event {
    let template = test.get_template_address(TEMPLATE);
    let before: Vec<ComponentAddress> = test
        .read_only_state_store()
        .get_components_by_template_address(template)
        .unwrap()
        .into_iter()
        .map(|c| c.0)
        .collect();
    test.execute_expect_success(
        test.transaction()
            .call_function(
                template,
                "new",
                args![
                    organizer.account,
                    title.to_string(),
                    Amount::from(price),
                    capacity,
                    refund_until,
                    doors,
                    end
                ],
            )
            .build_and_seal(&organizer.key),
        vec![organizer.proof.clone()],
    );
    // The store gives no ordering, so pick the component that did not exist before.
    let component = test
        .read_only_state_store()
        .get_components_by_template_address(template)
        .unwrap()
        .into_iter()
        .map(|c| c.0)
        .find(|c| !before.contains(c))
        .expect("new component");
    let tickets: ResourceAddress = read(test, component, "ticket_resource");
    let badges: ResourceAddress = read(test, component, "badge_resource");
    Event { component, tickets, badges }
}

fn read<T: for<'a> minicbor::Decode<'a, ()>>(test: &mut TemplateTest, c: ComponentAddress, method: &str) -> T {
    test.execute_expect_success(
        test.transaction().call_method(c, method, args![]).build_and_seal(test.secret_key()),
        vec![],
    );
    test.return_values()[0].decoded().unwrap()
}

type Info = (String, Amount, u32, u32, u32, bool, bool, u64, u64, u64, Amount);

fn info(test: &mut TemplateTest, e: &Event) -> Info {
    read(test, e.component, "info")
}

fn buy(test: &mut TemplateTest, buyer: &Actor, e: &Event) {
    test.execute_expect_success(
        test.transaction()
            .call_method(buyer.account, "withdraw", args![TARI_TOKEN, PRICE])
            .put_last_instruction_output_on_workspace("pay")
            .call_method(e.component, "buy", args![Workspace("pay"), buyer.account])
            .build_and_seal(&buyer.key),
        vec![buyer.proof.clone()],
    );
}

fn buy_amount_fails(test: &mut TemplateTest, buyer: &Actor, e: &Event, amount: u64, reason: &str) {
    let r = test.execute_expect_failure(
        test.transaction()
            .call_method(buyer.account, "withdraw", args![TARI_TOKEN, amount])
            .put_last_instruction_output_on_workspace("pay")
            .call_method(e.component, "buy", args![Workspace("pay"), buyer.account])
            .build_and_seal(&buyer.key),
        vec![buyer.proof.clone()],
    );
    assert_reject_reason(r, reason);
}

fn refund_tx(test: &mut TemplateTest, holder: &Actor, e: &Event) {
    test.execute_expect_success(
        test.transaction()
            .call_method(holder.account, "withdraw", args![e.tickets, 1u64])
            .put_last_instruction_output_on_workspace("t")
            .call_method(e.component, "refund", args![Workspace("t"), holder.account])
            .build_and_seal(&holder.key),
        vec![holder.proof.clone()],
    );
}

fn refund_fails(test: &mut TemplateTest, holder: &Actor, e: &Event, reason: &str) {
    let r = test.execute_expect_failure(
        test.transaction()
            .call_method(holder.account, "withdraw", args![e.tickets, 1u64])
            .put_last_instruction_output_on_workspace("t")
            .call_method(e.component, "refund", args![Workspace("t"), holder.account])
            .build_and_seal(&holder.key),
        vec![holder.proof.clone()],
    );
    assert_reject_reason(r, reason);
}

fn check_in(test: &mut TemplateTest, staff: &Actor, e: &Event, number: u64, attendee: &Actor) {
    test.execute_expect_success(
        test.transaction()
            .call_method(e.component, "check_in", args![number, attendee.account])
            .build_and_seal(&staff.key),
        vec![staff.proof.clone()],
    );
}

fn check_in_fails(test: &mut TemplateTest, staff: &Actor, e: &Event, number: u64, attendee: &Actor, reason: &str) {
    let r = test.execute_expect_failure(
        test.transaction()
            .call_method(e.component, "check_in", args![number, attendee.account])
            .build_and_seal(&staff.key),
        vec![staff.proof.clone()],
    );
    assert_reject_reason(r, reason);
}

fn call_fails(test: &mut TemplateTest, caller: &Actor, e: &Event, method: &str, reason: &str) {
    let r = test.execute_expect_failure(
        test.transaction().call_method(e.component, method, args![]).build_and_seal(&caller.key),
        vec![caller.proof.clone()],
    );
    assert_reject_reason(r, reason);
}

fn call_ok(test: &mut TemplateTest, caller: &Actor, e: &Event, method: &str) {
    test.execute_expect_success(
        test.transaction().call_method(e.component, method, args![]).build_and_seal(&caller.key),
        vec![caller.proof.clone()],
    );
}

fn tari(test: &TemplateTest, a: ComponentAddress) -> Amount {
    test.read_only_state_store()
        .get_vaults_for_account(a)
        .unwrap()
        .get(&TARI_TOKEN)
        .map(|v| v.balance())
        .unwrap_or_default()
}

fn held(test: &TemplateTest, a: ComponentAddress, res: ResourceAddress) -> Amount {
    test.read_only_state_store()
        .get_vaults_for_account(a)
        .unwrap()
        .get(&res)
        .map(|v| v.balance())
        .unwrap_or_default()
}

#[test]
fn create_sets_the_event_up() {
    let (mut test, _org, e) = setup();
    let i = info(&mut test, &e);
    assert_eq!(i.0, "Jazz Night");
    assert_eq!(i.1, Amount::from(PRICE));
    assert_eq!((i.2, i.3, i.4), (CAPACITY, 0, 0));
    assert!(!i.5 && !i.6);
    assert_eq!((i.7, i.8, i.9), (REFUND_UNTIL, DOORS, END));
    assert_eq!(i.10, Amount::zero());
}

#[test]
fn create_rejects_bad_inputs() {
    let mut test = TemplateTest::my_crate();
    set_epoch(&mut test, 10);
    let o = actor(&mut test);
    assert!(try_create(&mut test, &o, "", PRICE, 3, 100, 200, 300).is_err(), "empty title");
    assert!(try_create(&mut test, &o, "x", 0, 3, 100, 200, 300).is_err(), "zero price");
    assert!(try_create(&mut test, &o, "x", PRICE, 0, 100, 200, 300).is_err(), "zero capacity");
    assert!(try_create(&mut test, &o, "x", PRICE, 3, 5, 200, 300).is_err(), "refund window in the past");
    assert!(try_create(&mut test, &o, "x", PRICE, 3, 100, 50, 300).is_err(), "doors before refund close");
    assert!(try_create(&mut test, &o, "x", PRICE, 3, 100, 200, 200).is_err(), "end not after doors");
    assert!(try_create(&mut test, &o, "x", PRICE, 3, 100, 200, 300).is_ok());
}

#[test]
fn buy_mints_a_ticket_and_keeps_the_money_in_the_event() {
    let (mut test, _org, e) = setup();
    let fan = actor(&mut test);
    let before = tari(&test, fan.account);
    buy(&mut test, &fan, &e);
    assert_eq!(held(&test, fan.account, e.tickets), Amount::from(1u64));
    assert_eq!(tari(&test, fan.account), before - Amount::from(PRICE));
    let i = info(&mut test, &e);
    assert_eq!(i.3, 1);
    assert_eq!(i.10, Amount::from(PRICE));
}

#[test]
fn buy_rejects_wrong_amounts() {
    let (mut test, _org, e) = setup();
    let fan = actor(&mut test);
    buy_amount_fails(&mut test, &fan, &e, PRICE - 1, "payment must equal the ticket price");
    buy_amount_fails(&mut test, &fan, &e, PRICE + 1, "payment must equal the ticket price");
    assert_eq!(info(&mut test, &e).3, 0);
}

#[test]
fn buy_stops_when_sold_out() {
    let (mut test, _org, e) = setup();
    for _ in 0..CAPACITY {
        let f = actor(&mut test);
        buy(&mut test, &f, &e);
    }
    let late = actor(&mut test);
    buy_amount_fails(&mut test, &late, &e, PRICE, "sold out");
    assert_eq!(info(&mut test, &e).3, CAPACITY);
}

#[test]
fn buy_stops_after_the_event_ends() {
    let (mut test, _org, e) = setup();
    let fan = actor(&mut test);
    set_epoch(&mut test, END);
    buy_amount_fails(&mut test, &fan, &e, PRICE, "event has ended");
}

#[test]
fn refund_burns_the_ticket_and_returns_the_price() {
    let (mut test, _org, e) = setup();
    let fan = actor(&mut test);
    let start = tari(&test, fan.account);
    buy(&mut test, &fan, &e);
    refund_tx(&mut test, &fan, &e);
    assert_eq!(tari(&test, fan.account), start);
    assert_eq!(held(&test, fan.account, e.tickets), Amount::zero());
    assert_eq!(info(&mut test, &e).10, Amount::zero());
}

#[test]
fn refund_is_rejected_after_the_window() {
    let (mut test, _org, e) = setup();
    let fan = actor(&mut test);
    buy(&mut test, &fan, &e);
    set_epoch(&mut test, REFUND_UNTIL);
    refund_fails(&mut test, &fan, &e, "refund window has closed");
    assert_eq!(held(&test, fan.account, e.tickets), Amount::from(1u64));
}

#[test]
fn refund_rejects_a_bucket_that_is_not_a_ticket() {
    let (mut test, _org, e) = setup();
    let fan = actor(&mut test);
    buy(&mut test, &fan, &e);
    let r = test.execute_expect_failure(
        test.transaction()
            .call_method(fan.account, "withdraw", args![TARI_TOKEN, 1u64])
            .put_last_instruction_output_on_workspace("t")
            .call_method(e.component, "refund", args![Workspace("t"), fan.account])
            .build_and_seal(&fan.key),
        vec![fan.proof.clone()],
    );
    assert_reject_reason(r, "not a ticket for this event");
}

#[test]
fn a_ticket_can_be_given_away_and_the_new_holder_can_refund_it() {
    let (mut test, _org, e) = setup();
    let alice = actor(&mut test);
    let bob = actor(&mut test);
    buy(&mut test, &alice, &e);
    test.execute_expect_success(
        test.transaction()
            .call_method(alice.account, "withdraw", args![e.tickets, 1u64])
            .put_last_instruction_output_on_workspace("t")
            .call_method(bob.account, "deposit", args![Workspace("t")])
            .build_and_seal(&alice.key),
        vec![alice.proof.clone()],
    );
    assert_eq!(held(&test, alice.account, e.tickets), Amount::zero());
    let bob_before = tari(&test, bob.account);
    refund_tx(&mut test, &bob, &e);
    assert_eq!(tari(&test, bob.account), bob_before + Amount::from(PRICE));
}

#[test]
fn check_in_marks_the_ticket_used_and_mints_a_badge() {
    let (mut test, org, e) = setup();
    let fan = actor(&mut test);
    buy(&mut test, &fan, &e);
    set_epoch(&mut test, DOORS);
    check_in(&mut test, &org, &e, 1, &fan);
    assert_eq!(held(&test, fan.account, e.badges), Amount::from(1u64));
    let used: bool = {
        test.execute_expect_success(
            test.transaction().call_method(e.component, "is_checked_in", args![1u64]).build_and_seal(test.secret_key()),
            vec![],
        );
        test.return_values()[0].decoded().unwrap()
    };
    assert!(used);
    assert_eq!(info(&mut test, &e).4, 1);
}

#[test]
fn a_copied_ticket_fails_the_second_scan() {
    let (mut test, org, e) = setup();
    let fan = actor(&mut test);
    buy(&mut test, &fan, &e);
    set_epoch(&mut test, DOORS);
    check_in(&mut test, &org, &e, 1, &fan);
    let cheat = actor(&mut test);
    check_in_fails(&mut test, &org, &e, 1, &cheat, "ticket already used");
    assert_eq!(held(&test, cheat.account, e.badges), Amount::zero());
    assert_eq!(info(&mut test, &e).4, 1);
}

#[test]
fn a_stranger_cannot_check_anyone_in() {
    let (mut test, _org, e) = setup();
    let fan = actor(&mut test);
    let stranger = actor(&mut test);
    buy(&mut test, &fan, &e);
    set_epoch(&mut test, DOORS);
    check_in_fails(&mut test, &stranger, &e, 1, &stranger, "only the organizer or door staff can check in");
}

#[test]
fn door_staff_can_check_in_until_removed() {
    let (mut test, org, e) = setup();
    let fan = actor(&mut test);
    let fan2 = actor(&mut test);
    let staff = actor(&mut test);
    buy(&mut test, &fan, &e);
    buy(&mut test, &fan2, &e);
    test.execute_expect_success(
        test.transaction()
            .call_method(e.component, "add_staff", args![pubkey(&staff.key)])
            .build_and_seal(&org.key),
        vec![org.proof.clone()],
    );
    set_epoch(&mut test, DOORS);
    check_in(&mut test, &staff, &e, 1, &fan);
    test.execute_expect_success(
        test.transaction()
            .call_method(e.component, "remove_staff", args![pubkey(&staff.key)])
            .build_and_seal(&org.key),
        vec![org.proof.clone()],
    );
    check_in_fails(&mut test, &staff, &e, 2, &fan2, "only the organizer or door staff can check in");
}

#[test]
fn only_the_organizer_manages_staff() {
    let (mut test, _org, e) = setup();
    let stranger = actor(&mut test);
    let r = test.execute_expect_failure(
        test.transaction()
            .call_method(e.component, "add_staff", args![pubkey(&stranger.key)])
            .build_and_seal(&stranger.key),
        vec![stranger.proof.clone()],
    );
    assert_reject_reason(r, "only the organizer can do this");
}

#[test]
fn staff_list_is_capped() {
    let (mut test, org, e) = setup();
    for _ in 0..16 {
        let s = actor(&mut test);
        test.execute_expect_success(
            test.transaction().call_method(e.component, "add_staff", args![pubkey(&s.key)]).build_and_seal(&org.key),
            vec![org.proof.clone()],
        );
    }
    let extra = actor(&mut test);
    let r = test.execute_expect_failure(
        test.transaction().call_method(e.component, "add_staff", args![pubkey(&extra.key)]).build_and_seal(&org.key),
        vec![org.proof.clone()],
    );
    assert_reject_reason(r, "too many staff keys");
}

#[test]
fn check_in_only_works_while_doors_are_open() {
    let (mut test, org, e) = setup();
    let fan = actor(&mut test);
    buy(&mut test, &fan, &e);
    set_epoch(&mut test, DOORS - 1);
    check_in_fails(&mut test, &org, &e, 1, &fan, "doors are not open yet");
    set_epoch(&mut test, END);
    check_in_fails(&mut test, &org, &e, 1, &fan, "event has ended");
}

#[test]
fn check_in_rejects_unknown_and_refunded_tickets() {
    let (mut test, org, e) = setup();
    let fan = actor(&mut test);
    buy(&mut test, &fan, &e);
    refund_tx(&mut test, &fan, &e);
    set_epoch(&mut test, DOORS);
    check_in_fails(&mut test, &org, &e, 1, &fan, "unknown or refunded ticket");
    check_in_fails(&mut test, &org, &e, 99, &fan, "unknown or refunded ticket");
}

#[test]
fn a_used_ticket_cannot_be_refunded() {
    let (mut test, org, e) = setup();
    let fan = actor(&mut test);
    buy(&mut test, &fan, &e);
    set_epoch(&mut test, DOORS);
    check_in(&mut test, &org, &e, 1, &fan);
    // Doors are open, so the normal refund window is closed. Cancel is not possible either.
    refund_fails(&mut test, &fan, &e, "refund window has closed");
}

#[test]
fn the_badge_cannot_be_moved_out_of_the_account() {
    let (mut test, org, e) = setup();
    let fan = actor(&mut test);
    let other = actor(&mut test);
    buy(&mut test, &fan, &e);
    set_epoch(&mut test, DOORS);
    check_in(&mut test, &org, &e, 1, &fan);
    let r = test.execute_expect_failure(
        test.transaction()
            .call_method(fan.account, "withdraw", args![e.badges, 1u64])
            .put_last_instruction_output_on_workspace("b")
            .call_method(other.account, "deposit", args![Workspace("b")])
            .build_and_seal(&fan.key),
        vec![fan.proof.clone()],
    );
    let text = format!("{r:?}");
    assert!(text.to_lowercase().contains("denied") || text.to_lowercase().contains("withdraw"), "{text}");
    assert_eq!(held(&test, fan.account, e.badges), Amount::from(1u64));
    assert_eq!(held(&test, other.account, e.badges), Amount::zero());
}

#[test]
fn payout_waits_for_the_end_and_goes_to_the_organizer_once() {
    let (mut test, org, e) = setup();
    for _ in 0..2 {
        let f = actor(&mut test);
        buy(&mut test, &f, &e);
    }
    call_fails(&mut test, &org, &e, "payout", "event has not ended");
    set_epoch(&mut test, END);
    let stranger = actor(&mut test);
    call_fails(&mut test, &stranger, &e, "payout", "only the organizer can do this");
    let before = tari(&test, org.account);
    call_ok(&mut test, &org, &e, "payout");
    assert_eq!(tari(&test, org.account), before + Amount::from(PRICE * 2));
    assert_eq!(info(&mut test, &e).10, Amount::zero());
    call_fails(&mut test, &org, &e, "payout", "already paid out");
}

#[test]
fn payout_with_no_sales_is_rejected() {
    let (mut test, org, e) = setup();
    set_epoch(&mut test, END);
    call_fails(&mut test, &org, &e, "payout", "nothing to pay out");
}

#[test]
fn cancel_stops_sales_opens_refunds_forever_and_blocks_payout() {
    let (mut test, org, e) = setup();
    let fan = actor(&mut test);
    let start = tari(&test, fan.account);
    buy(&mut test, &fan, &e);
    call_ok(&mut test, &org, &e, "cancel");
    let late = actor(&mut test);
    buy_amount_fails(&mut test, &late, &e, PRICE, "event is cancelled");
    set_epoch(&mut test, END + 1000);
    call_fails(&mut test, &org, &e, "payout", "event was cancelled");
    refund_tx(&mut test, &fan, &e);
    assert_eq!(tari(&test, fan.account), start);
    assert!(info(&mut test, &e).5);
}

#[test]
fn cancel_is_organizer_only_and_only_before_doors() {
    let (mut test, org, e) = setup();
    let stranger = actor(&mut test);
    call_fails(&mut test, &stranger, &e, "cancel", "only the organizer can do this");
    set_epoch(&mut test, DOORS);
    call_fails(&mut test, &org, &e, "cancel", "too late to cancel");
}

#[test]
fn check_in_is_rejected_for_a_cancelled_event() {
    let (mut test, org, e) = setup();
    let fan = actor(&mut test);
    buy(&mut test, &fan, &e);
    call_ok(&mut test, &org, &e, "cancel");
    set_epoch(&mut test, DOORS);
    check_in_fails(&mut test, &org, &e, 1, &fan, "event is cancelled");
}

#[test]
fn takings_always_equal_sales_minus_refunds() {
    let (mut test, _org, e) = setup();
    let a = actor(&mut test);
    let b = actor(&mut test);
    let c = actor(&mut test);
    buy(&mut test, &a, &e);
    buy(&mut test, &b, &e);
    buy(&mut test, &c, &e);
    assert_eq!(info(&mut test, &e).10, Amount::from(PRICE * 3));
    refund_tx(&mut test, &b, &e);
    assert_eq!(info(&mut test, &e).10, Amount::from(PRICE * 2));
    refund_tx(&mut test, &a, &e);
    refund_tx(&mut test, &c, &e);
    assert_eq!(info(&mut test, &e).10, Amount::zero());
}

#[test]
fn a_ticket_for_one_event_cannot_refund_at_another() {
    let (mut test, org, e1) = setup();
    let e2 = create_event(&mut test, &org, "Other Night", PRICE, CAPACITY, REFUND_UNTIL, DOORS, END);
    let fan = actor(&mut test);
    buy(&mut test, &fan, &e1);
    let r = test.execute_expect_failure(
        test.transaction()
            .call_method(fan.account, "withdraw", args![e1.tickets, 1u64])
            .put_last_instruction_output_on_workspace("t")
            .call_method(e2.component, "refund", args![Workspace("t"), fan.account])
            .build_and_seal(&fan.key),
        vec![fan.proof.clone()],
    );
    assert_reject_reason(r, "not a ticket for this event");
}

#[test]
fn only_the_component_can_mint_tickets() {
    let (mut test, org, e) = setup();
    // The organizer has no mint right on the ticket resource. Trying to call a
    // component method that does not exist cannot mint either.
    let r = test.execute_expect_failure(
        test.transaction().call_method(e.component, "mint_ticket", args![]).build_and_seal(&org.key),
        vec![org.proof.clone()],
    );
    let _ = r;
    assert_eq!(info(&mut test, &e).3, 0);
}
