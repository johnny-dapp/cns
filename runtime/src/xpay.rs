use support::{decl_module, decl_storage, decl_event, StorageValue, StorageMap, dispatch::Result, Parameter, ensure};
use runtime_primitives::traits::{CheckedAdd, CheckedMul, As};
use system::ensure_signed;
use rstd::result;
use rstd::vec::Vec;
use crate::domain_service::{self, DomainName};
use parity_codec::Decode;

pub trait Trait: cennzx_spot::Trait + domain_service::Trait {
	type Item: Parameter;
	type ItemId: Parameter + CheckedAdd + Default + From<u8>;
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

pub type BalanceOf<T> = <T as generic_asset::Trait>::Balance;
pub type AssetIdOf<T> = <T as generic_asset::Trait>::AssetId;
pub type PriceOf<T> = (AssetIdOf<T>, BalanceOf<T>);

decl_storage! {
	trait Store for Module<T: Trait> as XPay {
		pub Items get(item): map T::ItemId => Option<T::Item>;
		pub ItemOwners get(item_owner): map T::ItemId => Option<DomainName>;
		pub ItemQuantities get(item_quantity): map T::ItemId => u32;
		pub ItemPrices get(item_price): map T::ItemId => Option<PriceOf<T>>;
		
		pub NextItemId get(next_item_id): T::ItemId;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

		pub fn create_item(origin, domain: DomainName, quantity: u32, item: T::Item, price_asset_id: AssetIdOf<T>, price_amount: BalanceOf<T>) -> Result {
			let origin = ensure_signed(origin)?;

			let item_id = Self::next_item_id();

			Self::ensure_ownwer(&domain, &origin)?;

			// The last available id serves as the overflow mark and won't be used.
			let next_item_id = item_id.checked_add(&1.into()).ok_or_else(||"No new item id is available.")?;

			<NextItemId<T>>::put(next_item_id);

			let price = (price_asset_id, price_amount);

			<Items<T>>::insert(item_id.clone(), item.clone());
			<ItemOwners<T>>::insert(item_id.clone(), domain);
			<ItemQuantities<T>>::insert(item_id.clone(), quantity);
			<ItemPrices<T>>::insert(item_id.clone(), price.clone());

			Self::deposit_event(RawEvent::ItemCreated(origin, item_id, quantity, item, price));

			Ok(())
		}

		pub fn add_item(origin, item_id: T::ItemId, quantity: u32) -> Result {
			let origin = ensure_signed(origin)?;

			<ItemQuantities<T>>::mutate(item_id.clone(), |q| *q = q.saturating_add(quantity));

			Self::deposit_event(RawEvent::ItemAdded(origin, item_id.clone(), Self::item_quantity(item_id)));

			Ok(())
		}

		pub fn remove_item(origin, item_id: T::ItemId, quantity: u32) -> Result {
			let origin = ensure_signed(origin)?;

			<ItemQuantities<T>>::mutate(item_id.clone(), |q| *q = q.saturating_sub(quantity));

			Self::deposit_event(RawEvent::ItemRemoved(origin, item_id.clone(), Self::item_quantity(item_id)));

			Ok(())
		}

		pub fn update_item(origin, item_id: T::ItemId, quantity: u32, price_asset_id: AssetIdOf<T>, price_amount: BalanceOf<T>) -> Result {
			let origin = ensure_signed(origin)?;

			ensure!(<Items<T>>::exists(item_id.clone()), "Item did not exist");

			<ItemQuantities<T>>::insert(item_id.clone(), quantity);

			let price = (price_asset_id, price_amount);
			<ItemPrices<T>>::insert(item_id.clone(), price.clone());

			Self::deposit_event(RawEvent::ItemUpdated(origin, item_id, quantity, price));

			Ok(())
		}

		pub fn purchase_item(origin, quantity: u32, item_id: T::ItemId, paying_asset_id: AssetIdOf<T>, max_total_paying_amount: BalanceOf<T>) -> Result {
			let origin = ensure_signed(origin)?;

			let new_quantity = Self::item_quantity(item_id.clone()).checked_sub(quantity).ok_or_else(||"Not enough quantity")?;
			let item_price = Self::item_price(item_id.clone()).ok_or_else(||"No item price")?;
			let seller = Self::item_owner(item_id.clone()).ok_or_else(||"No item owner")?;

			let total_price_amount = item_price.1.checked_mul(&As::sa(quantity as u64)).ok_or_else(||"Total price overflow")?;

			Self::make_transfer(&origin, paying_asset_id, max_total_paying_amount, &seller, item_price.0, total_price_amount)?;

			<ItemQuantities<T>>::insert(item_id.clone(), new_quantity);

			Self::deposit_event(RawEvent::ItemSold(origin, item_id, quantity));

			Ok(())
		}

		pub fn transfer(
			origin,
			from_asset: AssetIdOf<T>,
			from_amount: BalanceOf<T>,
			to_domain: DomainName,
			to_asset: AssetIdOf<T>,
			to_amount: BalanceOf<T>
		) {
			let origin = ensure_signed(origin)?;
			Self::make_transfer(&origin, from_asset, from_amount, &to_domain, to_asset, to_amount)?;
		}
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		<T as Trait>::Item,
		<T as Trait>::ItemId,
		Price = PriceOf<T>,
	{
		/// New item created. (transactor, item_id, quantity, item, price)
		ItemCreated(AccountId, ItemId, u32, Item, Price),
		/// More items added. (transactor, item_id, new_quantity)
		ItemAdded(AccountId, ItemId, u32),
		/// Items removed. (transactor, item_id, new_quantity)
		ItemRemoved(AccountId, ItemId, u32),
		/// Item updated. (transactor, item_id, new_quantity, new_price)
		ItemUpdated(AccountId, ItemId, u32, Price),
		/// Item sold. (transactor, item_id, quantity)
		ItemSold(AccountId, ItemId, u32),
	}
);

impl<T: Trait> Module<T> {
	fn ensure_ownwer(domain: &DomainName, owner: &T::AccountId) -> Result {
		let detail = domain_service::Module::<T>::domains(domain).ok_or_else(|| "Domain not exist")?;
		if *owner == detail.owner {
			return Ok(());
		}
		return Err("Not owner");
	}

	fn resolve_domain(domain: &DomainName) -> result::Result<T::AccountId, &'static str> {
		let detail = domain_service::Module::<T>::domains(domain).ok_or_else(|| "Domain not exist")?;
		let addr = detail.addr.ok_or_else(|| "Domain not published")?;
		Decode::decode(&mut &addr[..]).ok_or_else(|| "Not address")
	}

	fn make_transfer(
		from: &T::AccountId,
		from_asset: AssetIdOf<T>,
		from_amount: BalanceOf<T>,
		to_domain: &DomainName,
		to_asset: AssetIdOf<T>,
		to_amount: BalanceOf<T>
	) -> Result {
		let to_account = Self::resolve_domain(to_domain)?;
		if from_asset == to_asset {
			// Same asset, GA transfer

			<generic_asset::Module<T>>::make_transfer_with_event(&from_asset, &from, &to_account, to_amount)?;
		} else {
			// Different asset, CENNZX-Spot transfer

			<cennzx_spot::Module<T>>::make_asset_swap_output(
				&from,            // buyer
				&to_account,      // recipient
				&from_asset,  		// asset_sold
				&to_asset,       	// asset_bought
				to_amount,       	// buy_amount
				from_amount,  		// max_paying_amount
				<cennzx_spot::Module<T>>::fee_rate() // fee_rate
			)?;
		}

		Ok(())
	}
}