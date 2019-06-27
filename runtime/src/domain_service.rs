/// A runtime module template with necessary imports

/// Feel free to remove or edit this file as needed.
/// If you change the name of this file, make sure to update its references in runtime/src/lib.rs
/// If you remove this file, you can remove those references


/// For more guidance on Substrate modules, see the example module
/// https://github.com/paritytech/substrate/blob/master/srml/example/src/lib.rs

use support::{decl_module, decl_storage, decl_event, StorageValue, StorageMap, dispatch::Result};
use system::ensure_signed;
use parity_codec::{Decode, Encode};
use runtime_primitives::traits::As;
use rstd::prelude::*;

/// The module's configuration trait.
pub trait Trait: system::Trait {
	// TODO: Add other types and constants required configure this module.

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

pub type DomainAddr = Vec<u8>;
pub type DomainName = Vec<u8>;

#[cfg_attr(feature = "std", derive(Debug))]
#[derive(Clone, Encode, Decode, Default, PartialEq, Eq)]
pub struct DomainDetail<AccountId, BlockNumber> {
	pub owner: AccountId,
	pub expire: BlockNumber,
	pub addr: Option<DomainAddr>,
}

#[cfg_attr(feature = "std", derive(Debug))]
#[derive(Clone, Encode, Decode, Default, PartialEq, Eq)]
pub struct Bid<AccountId> {
	pub bidder: AccountId,
	pub name: DomainName,
	pub amount: u128,
}

#[cfg_attr(feature = "std", derive(Debug))]
#[derive(Clone, Encode, Decode, Default, PartialEq, Eq)]
pub struct BidInfo<AccountId, BlockNumber> {
	pub bid: Bid<AccountId>,
	pub end: BlockNumber,
}

// This module's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as DomainService {
		Domains get(domains): map DomainName => Option<DomainDetail<T::AccountId, T::BlockNumber>>;
		Owners get(owners): map T::AccountId => Option<Vec<DomainName>>;
		Bids get(bids): map DomainName => Option<BidInfo<T::AccountId, T::BlockNumber>>;
	}
}

decl_module! {
	/// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		// Initializing events
		// this is needed only if you are using events in your module
		fn deposit_event<T>() = default;

		pub fn bid(origin, name: DomainName, amount: u128) -> Result {
			let who = ensure_signed(origin)?;

			if let Some(bid_info) = <Bids<T>>::get(name.clone()) {
				if amount <= bid_info.bid.amount {
					return Err("bid amount too small")
				}
			}

			let now = <system::Module<T>>::block_number();

			<Bids<T>>::insert(name.clone(), BidInfo {
				bid: Bid {
					bidder: who,
					name: name,
					amount: amount,
				},
				end: add_block_number_by(now, 10),
			});

			Ok(())
		}

		pub fn update(origin, name: DomainName, addr: Option<DomainAddr>) -> Result {
			let domain_detail_record = {
				if let Some(bid_info) = <Bids<T>>::take(name.clone()) {
					//TODO: transfer money to pool
					Some(DomainDetail {
						owner: bid_info.bid.bidder,
						expire: add_block_number_by(bid_info.end, 1000),
						addr: None,
					})
				} else {
					<Domains<T>>::get(name.clone())
				}
			};

			if let Some(domain_detail) = domain_detail_record {
				let who = ensure_signed(origin)?;
				if who != domain_detail.owner {
					return Err("not owner")
				}

				let owner_domains = if let Some(mut domain_names) = <Owners<T>>::take(who.clone()) {
					if !domain_names.contains(&name) {
						domain_names.push(name.clone());
					}
					domain_names
				} else {
					vec![name.clone()]
				};
				<Owners<T>>::insert(who.clone(), owner_domains);

				<Domains<T>>::insert(name, DomainDetail {
					owner: domain_detail.owner,
					expire: domain_detail.expire,
					addr: addr,
				});

				Ok(())
			} else {
				Err("domain does not exist")
			}
		}

		pub fn transfer() {

		}
	}
}

fn add_block_number_by<B: As<u64>>(block_number: B, by: u64) -> B {
	B::sa(block_number.as_() + by)
}

decl_event!(
	pub enum Event<T> where AccountId = <T as system::Trait>::AccountId {
		// Just a dummy event.
		// Event `Something` is declared with a parameter of the type `u32` and `AccountId`
		// To emit this event, we call the deposit funtion, from our runtime funtions
		SomethingStored(u32, AccountId),
	}
);
