use core::array::ArrayTrait;
use core::serde::Serde;
use core::pedersen::PedersenTrait;
use core::pedersen::HashState;
use core::hash::{HashStateTrait, HashStateExTrait};

// Serialization only works for up to 4 elements. Lets hardcode for 5 heehee
impl TupleSize5Serde<
    E0,
    E1,
    E2,
    E3,
    E4,
    +Serde<E0>,
    +Drop<E0>,
    +Serde<E1>,
    +Drop<E1>,
    +Serde<E2>,
    +Drop<E2>,
    +Serde<E3>,
    +Drop<E3>,
    +Serde<E4>,
    +Drop<E4>
> of Serde<(E0, E1, E2, E3, E4)> {
    fn serialize(self: @(E0, E1, E2, E3, E4), ref output: Array<felt252>) {
        let (e0, e1, e2, e3, e4) = self;
        e0.serialize(ref output);
        e1.serialize(ref output);
        e2.serialize(ref output);
        e3.serialize(ref output);
        e4.serialize(ref output)
    }
    fn deserialize(ref serialized: Span<felt252>) -> Option<(E0, E1, E2, E3, E4)> {
        Option::Some(
            (
                Serde::deserialize(ref serialized)?,
                Serde::deserialize(ref serialized)?,
                Serde::deserialize(ref serialized)?,
                Serde::deserialize(ref serialized)?,
                Serde::deserialize(ref serialized)?
            )
        )
    }
}

#[derive(Serde, Drop, Clone, Debug)]
struct Account {
    name: ByteArray,
    amount: u64,
}

#[derive(Serde, Drop, Clone, Debug)]
struct Event {
    from: ByteArray,
    to: ByteArray,
    amount: u64,
}

#[derive(Serde, Drop, Clone, Debug)]
struct HyleOutput {
    version: u32,
    initial_state: felt252,
    next_state: felt252,
    origin: ByteArray,
    caller: ByteArray,
    tx_hash: felt252,
    program_outputs: Event
}

fn get_account(balances: @Array<Account>, account_name: @ByteArray) -> Option<@Account> {
    let nb_of_accounts: usize = balances.len();
    let mut n = 0;

    let account = loop{
        if n >= nb_of_accounts {
            break Option::None;
        }
        let a: @Account = balances.at(n);
        if (a.name == account_name) {
            break Option::Some(a);
        }
        n += 1;
    };
    account
}

fn update_account(balances: Array<Account>, new_account: Account) -> Array<Account> {
    let nb_of_accounts: usize = balances.len();
    let mut n = 0;
    let mut new_balances: Array<Account> = array![];

    while n < nb_of_accounts {
        let a: @Account = balances.at(n);
        if (a.name == @new_account.name) {
            new_balances.append(new_account.clone());
        } else {
            new_balances.append(a.clone());
        }
        n += 1;
    };
    new_balances
}

fn compute_state(balances: @Array<Account>) -> felt252 {
    let nb_of_accounts: usize = balances.len();
    let mut n = 0;
    let first_element = 1;
    let mut state = PedersenTrait::new(first_element);    

    while n < nb_of_accounts { // Manually hashing all Accounts...
        let a: @Account = balances.at(n);
        let mut serialized_account: Array<felt252> = ArrayTrait::new();
        a.serialize(ref serialized_account);
        while let Option::Some(value) = serialized_account.pop_front() { // ... manually hashing all values of an account...
            state = state.update(value);
        };
        n += 1;
    };
   
    state.finalize()
}


fn main(input: Array<felt252>) -> Array<felt252> {
    // bob --> to_hex = 626f62 --> to_int = 6451042 --> to_serialized = [0 6451042 3]
    // alice --> to_hex = 616c696365a --> to_int = 418430673765 = to_serialized = [0 418430673765 5]

    let mut input = input.span();

    let (mut balances, amount, from, to, initial_state): (Array<Account>, u64, ByteArray, ByteArray, felt252) = Serde::deserialize(ref input).unwrap();

    // Initial state compute
    let computed_initial_state = compute_state(@balances);
    assert!(computed_initial_state == initial_state, "Initial state mismatch");

    // Get olds balances
    let from_balance = match get_account(@balances, @from) {
        Option::Some(x) => x.amount,
        Option::None => panic!("Unable to find the sender")
    };

    let to_balance = match get_account(@balances, @to) {
        Option::Some(x) => x.amount,
        Option::None => {
            let new_account = Account {name: to.clone(), amount: 0_u64};
            balances.append(new_account);
            @0_u64
        }
    };
  
    // Change balances
    assert!(*from_balance >= amount, "Does not have enough funds"); // Potential overflow

    let balances1 = update_account(balances, Account {name: from.clone(), amount: *from_balance - amount});
    let balances2 = update_account(balances1, Account {name: to.clone(), amount: *to_balance + amount});

    // Next state compute
    let computed_final_state = compute_state(@balances2);

    // Transfer event
    let event = Event { from: from.clone(), to: to.clone(), amount: amount };

    // HyleOutput
    let hyle_output = HyleOutput {
        version: 1,
        initial_state: initial_state,
        next_state: computed_final_state,
        origin: "",
        caller: "",
        tx_hash: 0,
        program_outputs: event,
    };

    let mut output: Array<felt252> = ArrayTrait::new();
    hyle_output.serialize(ref output);
    output
}
