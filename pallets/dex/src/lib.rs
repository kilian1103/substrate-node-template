#![cfg_attr(not(feature = "std"), no_std)]


// Re-export pallet items so that they can be accessed from the crate namespace.
pub use frame_system::pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use codec::HasCompact;
	use frame_support::dispatch::RawOrigin;
	use frame_support::pallet_prelude::*;
	use frame_support::PalletId;
	use frame_support::sp_runtime::{
		ArithmeticError,
		TokenError, traits::{
			AccountIdConversion, AtLeast32BitUnsigned, Bounded, CheckedAdd, CheckedSub, Saturating, StaticLookup, Zero,
		},
	};
	use frame_support::traits::ReservableCurrency;
	use frame_system::pallet_prelude::*;
	use pallet_assets;
	use sp_std::vec::Vec;

// Step 3.1 will include this in `Cargo.toml`

	#[pallet::config]
	/// The module configuration trait.
	pub trait Config<I: 'static = ()>: frame_system::Config + pallet_assets::Config {
		/// The overarching event type.
		type Event: From<Event<Self, I>> + IsType<<Self as frame_system::Config>::Event>;


		/// The currency mechanism.
		type Currency: ReservableCurrency<Self::AccountId>;

		/// The treasury's pallet id, used for deriving its sovereign account ID.
		#[pallet::constant]
		type PalletId: Get<PalletId>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		AddLiquidity { owner: T::AccountId, asset_id_1: T::AssetId, amount1: T::Balance, asset_id_2: T::AssetId, amount2: T::Balance },
		RemoveLiquidity { owner: T::AccountId, asset_id_1: T::AssetId, amount1: T::Balance, asset_id_2: T::AssetId, amount2: T::Balance },
		Transfer {
			asset_id: T::AssetId,
			from: T::AccountId,
			to: T::AccountId,
			amount: T::Balance,
		},
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// Account balance must be greater than or equal to the transfer amount.
		BalanceLow,

	}

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T, I = ()>(_);

	#[pallet::storage]
	/// Details of an asset.
	pub(super) type Assets<T: Config<I>, I: 'static = ()> = StorageMap<
		_,
		Blake2_128Concat,
		T::AssetId, T::Balance, ValueQuery>;


	#[pallet::storage]
	/// The holdings of a specific account for a specific asset
	pub(super) type Account<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::AssetId,
		T::Balance,
		ValueQuery>;

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		#[pallet::weight(1_000)]
		pub fn provide_liquidity(origin: OriginFor<T>, tokenId1: T::AssetId, amount1: T::Balance, tokenId2: T::AssetId, amount2: T::Balance) -> DispatchResult {

			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			let sender = ensure_signed(origin.clone())?;

			// Check balances
			ensure!(pallet_assets::Pallet::<T>::balance(tokenId1, &sender) >= amount1, Error::<T,I>::BalanceLow);
			ensure!(pallet_assets::Pallet::<T>::balance(tokenId2, &sender) >= amount2, Error::<T,I>::BalanceLow);

			// Transfer funds to treasury from msg sender
			pallet_assets::Pallet::<T>::transfer(origin.clone(), tokenId1, T::PalletId::get().into_account_truncating(), amount1);
			pallet_assets::Pallet::<T>::transfer(origin, tokenId2, T::PalletId::get().into_account_truncating(), amount2);

			// Get liquidity balances
			let token1_balance = Assets::<T, I>::get(&tokenId1);
			let token2_balance = Assets::<T, I>::get(&tokenId2);
			// add liquidity
			let diff1 = token1_balance + amount1;
			let diff2 = token2_balance + amount2;
			// update liquidity pool
			Assets::<T, I>::insert(&tokenId1, diff1);
			Assets::<T, I>::insert(&tokenId2, diff2);

			// emit event
			Self::deposit_event(Event::AddLiquidity { owner: sender, asset_id_1: tokenId1, amount1: amount1, asset_id_2: tokenId2, amount2: amount2 });
			Ok(())
		}
		#[pallet::weight(10_000)]
		pub fn remove_liquidity(origin: OriginFor<T>, tokenId1: T::AssetId, amount1: T::Balance, tokenId2: T::AssetId, amount2: T::Balance) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://docs.substrate.io/v3/runtime/origins

			let sender = ensure_signed(origin)?;

			// Get liquidity balances
			let token1_balance = Assets::<T, I>::get(&tokenId1);
			let token2_balance = Assets::<T, I>::get(&tokenId2);
			// Verify that liquidity pool has sufficient balances
			ensure!(token1_balance >= amount1, Error::<T,I>::BalanceLow);
			ensure!(token2_balance >= amount2, Error::<T,I>::BalanceLow);

			// Remove liquidity
			let diff1 = token1_balance - amount1;
			let diff2 = token2_balance - amount2;

			// update liquidity pool
			Assets::<T, I>::insert(&tokenId1, diff1);
			Assets::<T, I>::insert(&tokenId2, diff2);

			// mov funds from treasury to msg sender
			let treasury = Self::account_id();
			let lookup = <T as frame_system::Config>::Lookup::unlookup(sender.clone());
			pallet_assets::Pallet::<T>::transfer(RawOrigin::Signed(treasury.clone()).into(), tokenId1, lookup.clone(), amount1);
			pallet_assets::Pallet::<T>::transfer(RawOrigin::Signed(treasury).into(), tokenId2, lookup, amount2);

			// Emit an event that liquidity was removed
			Self::deposit_event(Event::RemoveLiquidity { owner: sender.clone(), asset_id_1: tokenId1, amount1: amount1, asset_id_2: tokenId2, amount2: amount2 });
			Ok(())
		}

		#[pallet::weight(1_000)]
		pub fn exchange_token(origin: OriginFor<T>, tokenId1: T::AssetId, amount1: T::Balance, tokenId2: T::AssetId) -> DispatchResult {
			let sender = ensure_signed(origin.clone())?;

			// get exchange rate of token 2 by providing token 1
			let exchanged_tokens = Self::get_exchange_rate(tokenId1, amount1, tokenId2);


			let treasury = Self::account_id();
			let lookup = <T as frame_system::Config>::Lookup::unlookup(sender.clone());

			// send token 1 from msg sender to treasury
			pallet_assets::Pallet::<T>::transfer(origin, tokenId1.clone(), T::PalletId::get().into_account_truncating(), amount1);
			// send token 2 from teasury to msg sender
			pallet_assets::Pallet::<T>::transfer(RawOrigin::Signed(treasury.clone()).into(), tokenId2, lookup.clone(), exchanged_tokens);

			// update liquidity pool book
			let token1_balance = Assets::<T, I>::get(&tokenId1);
			let token2_balance = Assets::<T, I>::get(&tokenId2);
			let diff1 = token1_balance + amount1;
			let diff2 = token2_balance - exchanged_tokens;
			Assets::<T, I>::insert(&tokenId1, diff1);
			Assets::<T, I>::insert(&tokenId2, diff2);

			// emit transfer of exchanged token event
			Self::deposit_event(Event::Transfer {
				asset_id: tokenId1,
				from: sender,
				to: treasury,
				amount: amount1,
			});
			Ok(())
		}
	}

	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		// Add public immutables and private mutables.

		/// The account ID of the treasury pot.
		///
		/// This actually does computation. If you need to keep using it, then make sure you cache the
		/// value and only call this once.
		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		/// ask how much of token 2 can be acquired for a given amount of token 1
		pub fn get_exchange_rate(assetId_input: T::AssetId, amount_input: T::Balance, assetId_output: T::AssetId) -> T::Balance {
			let prod = Assets::<T, I>::get(&assetId_input) * Assets::<T, I>::get(&assetId_output);
			let return_value = prod / (Assets::<T, I>::get(&assetId_input) + amount_input) - Assets::<T, I>::get(&assetId_output);
			return_value
		}

		pub fn compute_staking_reward() -> T::Balance {
			//Pseudo code: Keep track of assets of specific users and compute liquidity fee e.g. 0.01 * T::Balance
			todo!("Implement the staking reward function.")
		}
	}
}



