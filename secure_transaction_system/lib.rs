#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod payment_contract {
    use core::ops::Add;

    use ink::env:: hash;
    use ink::prelude::vec::Vec;
    use ink::{
        env::{
            block_timestamp,
            hash::{HashOutput, Sha2x256},
            DefaultEnvironment,
        },
        storage::Mapping,
    };

    const ATTEMPTS_LIMIT: u8 = 3;

    #[ink(storage)]
    pub struct PaymentContract {
        payment_records: Mapping<Hash, PaymentInfo>,
        threshold_value: Balance,
        admin: AccountId,
        expiry_time: Timestamp,
        salt: u64,
    }

    // ---------------------- Custom Struct---------------------------

    #[derive(scale::Decode, scale::Encode, Debug, Clone)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]

    pub struct PaymentInfo {
        sender: AccountId,
        receiver: AccountId,
        amount: Balance,
        otp: u32,
        otp_attempts: u8,
        recorded_time: u64,
        status: PaymentStatus,
    }

    // ------------------------EVENT-----------------------------
    #[ink(event)]
    pub struct SecurePaymentRequested {
        #[ink(topic)]
        sender: AccountId,
        #[ink(topic)]
        receiver: AccountId,
        amount: Balance,
        payment_id: Hash,
        otp:u32
    }

    #[ink(event)]
    pub struct SecurePaymentInfo {
        #[ink(topic)]
        sender: AccountId,
        #[ink(topic)]
        receiver: AccountId,
        amount: Balance,
        payment_id: Hash,
        status: PaymentStatus,
    }

    #[ink(event)]
    pub struct ViewPaymentInfo {
        info: PaymentInfo,
    }

    // ------------------------------Error---------------------------
    pub type Result<T> = core::result::Result<T, Error>;

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        /// Returned if not enough balance to fulfill the request
        BalanceMismatch,
        /// Returned if payment request expired
        TimeLimitExceeded,
        // Caller is not receiver of payment request
        InvalidReceiver,
        // Caller is not sender of payment record
        InvalidSender,
        // OTP doesn't match
        WrongOTP,
        // No Attempts left
        AttemptsExceedLimit,
        // Already exists Transaction Id
        TxnIDAlreadExists,
        // Payment record not found
        PaymentRecordMissing,
        // Payment record not Expired
        NotAllowed,
        // RESET not typed correctly,
        IncorrectREFUND,
        // Value overflowed
        Overflow,
        // Admin only operation
        InvalidCaller,
        // Below Threshold value
        BelowThresholdValue,
        // Not in waiting period
        AlreadyReceivedPayment,
        // Zero balance not accepted
        ZeroBalance,
    }

    #[derive(Debug, PartialEq, Eq, scale::Decode, scale::Encode, Clone)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]

    enum PaymentStatus {
        // Payment expired after 24 hours
        Expired,
        // Payment waiting receiver's input Key
        Waiting,
       // Payment failed due to too many input attempts
        AllAttemptsFailed,
        // Payment success due to inputting correct key
        Success,
        // Payment Refunded
        Refunded,
    }

    // ------------------------------Impl Contract---------------------------

    impl PaymentContract {
        #[ink(constructor)]
        pub fn new(admin: AccountId) -> Self {
            Self {
                payment_records: Mapping::default(),
                threshold_value: u128::pow(10, 14),
                admin,
                expiry_time: 86_400_000,
                salt: 0,
            }
        }

        fn create_payment_info(
            &self,
            receiver: AccountId,
            sender: AccountId,
            amount: Balance,
            otp: u32,
        ) -> PaymentInfo {
            PaymentInfo {
                sender,
                receiver,
                amount,
                otp,
                otp_attempts: 1,
                recorded_time: block_timestamp::<DefaultEnvironment>(),
                status: PaymentStatus::Waiting,
            }
        }
      
        /// Handles payment_info from sender
        #[ink(message, payable)]
        pub fn send_payment(&mut self, receiver: AccountId, amount: Balance) -> Result<()> {
            let caller = self.env().caller();

            // Check the Locked amount
            let amount_funded = self.env().transferred_value();
            if amount != amount_funded {
                return Err(Error::BalanceMismatch);
            }

            // zero balance not accepted
            if amount_funded == 0 {
                return Err(Error::ZeroBalance);
            }

            // convert the units
          

            // Check if amount exceeds the threshold value
            if amount < self.threshold_value {
                return Err(Error::BelowThresholdValue);
            }

            // create fixed length random OTP (9 digits)
            let otp: u32 = self.get_pseudo_random();

            // Get payment_info and transaction_id
            let payment_info = self.create_payment_info(receiver, caller, amount, otp);


            // let transaction_id = self.get_transaction_id(&payment_info);
            let transaction_id = self.get_transaction_id(&payment_info);

            // Insert the payment record
            if self
                .payment_records
                .insert(transaction_id, &payment_info)
                .is_some()

            {
                return Err(Error::TxnIDAlreadExists);
            } 
            else {
                // Emit event for payment record request
                self.env().emit_event(SecurePaymentRequested {
                    sender: caller,
                    receiver,
                    amount,
                    payment_id: transaction_id,
                    otp
                });
            }   
            Ok(())
        }
        fn get_pseudo_random(&mut self) -> u32 {
            let seed = self.env().block_timestamp();

            let mut input: Vec<u8> = Vec::new();
            input.extend_from_slice(&seed.to_be_bytes());
            input.extend_from_slice(&self.salt.to_be_bytes());

            let mut output = <hash::Keccak256 as hash::HashOutput>::Type::default();
            ink::env::hash_bytes::<hash::Keccak256>(&input, &mut output);

            self.salt = self.salt.wrapping_add(1);

            let mut part1 = output[0] as u32;
            if part1 < 100 {
                part1 = part1.wrapping_add(100);
            }
            let mut part2 = output[1] as u32;
            if part2 < 100 {
                part2 = part2.wrapping_add(100);
            }
            let mut part3 = output[2] as u32;
            if part3 < 100 {
                part3 = part3.wrapping_add(100);
            }

            let prefix = part1.wrapping_mul(1000).wrapping_add(part2);
            prefix.wrapping_mul(1000).wrapping_add(part3)
        }

        #[ink(message)]
        pub fn get_refund(&mut self, payment_id: Hash) -> Result<()> {
            let payment_info = self.payment_records.get(payment_id);
            if payment_info.is_none() {
                return Err(Error::PaymentRecordMissing);
            }
            let mut payment_info = payment_info.unwrap();

            let caller = self.env().caller();
            if caller != payment_info.sender {
                return Err(Error::InvalidSender);
            }

            if self.is_expired(payment_info.recorded_time) && payment_info.status!=PaymentStatus::Refunded && payment_info.status!=PaymentStatus::Success {

                payment_info.status = PaymentStatus::Refunded;

                self.env()
                    .transfer(payment_info.sender, payment_info.amount)
                    .unwrap();

                self.payment_records.insert(payment_id, &payment_info);

                self.env().emit_event(SecurePaymentInfo {
                    sender: payment_info.sender,
                    receiver: payment_info.receiver,
                    amount: payment_info.amount,
                    payment_id,
                    status: payment_info.status,
                });
                Ok(())
            } else {
                Err(Error::NotAllowed)
            }
      
        }
             
         /// Handles payment_id & OTP from receiver for verification
        #[ink(message)]
        pub fn receive_payment(&mut self, payment_id: Hash, sent_otp: u32) -> Result<()> {

            let payment_info = self.payment_records.get(payment_id);
            
            if payment_info.is_none() {
                return Err(Error::PaymentRecordMissing);
            }

            let mut payment_info = payment_info.unwrap();

            let status = payment_info.status.clone();

            if status != PaymentStatus::Waiting || status == PaymentStatus::Refunded
            { 
                return Err(Error::AlreadyReceivedPayment);
            }

            let caller = self.env().caller();
            if caller != payment_info.receiver {
                return Err(Error::InvalidReceiver);
            }

            // Check if payment has expired
            if self.is_expired(payment_info.recorded_time) {
                payment_info.status = PaymentStatus::Expired;
                self.env().emit_event(SecurePaymentInfo {
                    sender: payment_info.sender,
                    receiver: payment_info.receiver,
                    amount: payment_info.amount,
                    payment_id,
                    status: payment_info.status,
                });
                return Err(Error::TimeLimitExceeded);
            }

            // match the otps
            if payment_info.otp != sent_otp {
                use crate::payment_contract::ATTEMPTS_LIMIT;

                // if attempts exceeded the decided limit
                if payment_info.otp_attempts > ATTEMPTS_LIMIT {
                    self.all_attempts_done( &mut payment_info, payment_id)
                } else {
                    self.one_attempt_done( &mut payment_info, payment_id)
                }
            } else {

                // transfer amount to receiver
                let amount=self.get_amount(&payment_info);
                self.env()
                    .transfer(payment_info.receiver, amount)
                    .unwrap();

                payment_info.status = PaymentStatus::Success;
                
                self.payment_records.insert(payment_id, &payment_info);

                // emit success event
                self.env().emit_event(SecurePaymentInfo {
                    sender: payment_info.sender,
                    receiver: payment_info.receiver,
                    amount: payment_info.amount,
                    payment_id,
                    status: payment_info.status,
                });
                Ok(())
            }
        }

         #[ink(message)]
        pub fn set_threshold_amount(&mut self, threshold_value: Balance) -> Result<()> {
            if self.admin == self.env().caller() {
                self.threshold_value = threshold_value;
                Ok(())
            } else {
                Err(Error::InvalidCaller)
            }
        }

        #[ink(message)]
        pub fn view_payment_expiry_time(&self,payment_id: Hash) -> Timestamp{
            let time = self.expiry_time;
            let payment_info=self.payment_records.get(payment_id).unwrap();
            let payment_created_time=payment_info.recorded_time;
            payment_created_time.add(time)
        }

        #[ink(message)]
        pub fn set_expiry_period(&mut self, time: Timestamp) -> Result<()> {
            if self.admin == self.env().caller() {
                self.expiry_time = time;
                Ok(())
            } else {
                Err(Error::InvalidCaller)
            }
        }

        // #[ink(message)]
        // pub fn view_current_time(&self) -> Timestamp {
        //     let time = self.env().block_timestamp();
        //     ink::env::debug_println!("{:?}", time);
        //     time
        // }

         #[ink(message)]
        pub fn view_payment_record(&self, payment_id: Hash) -> PaymentInfo {
            let payment_info = self.payment_records.get(payment_id).unwrap();
            payment_info
        }



        fn all_attempts_done(&self, payment_info: &mut PaymentInfo, payment_id: Hash) -> Result<()> {
            // refund payment to sender
            self.env()
                .transfer(payment_info.sender, payment_info.amount)
                .unwrap();
            self.payment_records.remove(payment_id);

            payment_info.status = PaymentStatus::AllAttemptsFailed;

            self.env().emit_event(SecurePaymentInfo {
                sender: payment_info.sender,
                receiver: payment_info.receiver,
                amount: payment_info.amount,
                payment_id,
                status: payment_info.status.clone(),
            });
            Err(Error::AttemptsExceedLimit)
        }

        fn one_attempt_done(&self, payment_info: &mut PaymentInfo, payment_id: Hash) -> Result<()> {
            // one more attempt done
            let otps = payment_info.otp_attempts.checked_add(1);
            if otps.is_none() {
                return Err(Error::Overflow);
            } else {
                payment_info.otp_attempts = otps.unwrap();
            }

            payment_info.status = PaymentStatus::Waiting;

            self.env().emit_event(SecurePaymentInfo {
                sender: payment_info.sender,
                receiver: payment_info.receiver,
                amount: payment_info.amount,
                payment_id,
                status: payment_info.status.clone(),
            });
            Err(Error::WrongOTP)
        }

        fn is_expired(&self, recorded_time: u64) -> bool {
            // 1 day has 86,400 seconds
            let expiry_time=self.expiry_time;
            block_timestamp::<DefaultEnvironment>() > recorded_time.checked_add(expiry_time).unwrap()
        }
        
        /// Returns the payment_id of payment_info
        fn get_transaction_id(&self, payment_info: &PaymentInfo) -> Hash {
            let encodable = payment_info;
            let mut payment_id = <Sha2x256 as HashOutput>::Type::default();
            ink::env::hash_encoded::<Sha2x256, _>(&encodable, &mut payment_id);
            Hash::from(payment_id)
        }

        fn get_amount(&self,payment_info: &PaymentInfo)->Balance{
            let encodable = payment_info;
           let amount=encodable.amount;
            amount
           }
    }
      
}   

    // #[cfg(test)]
    // mod tests {
    //     use ink::env::{
    //         debug_print,
    //         test::{self, recorded_events, set_block_timestamp, transfer_in},
    //         DefaultEnvironment,
    //     };

    //     use super::*;
    //     use ink::{
    //         env::{
    //             test::{
    //                 default_accounts, set_account_balance, set_callee, set_caller, DefaultAccounts,
    //             },
    //             transfer,
    //         },
    //         primitives::AccountId,
    //     };

    //     #[ink::test]
    //     fn new_works() {
    //         let admin = AccountId::from([0x03; 32]);
    //         let sts_contract = PaymentContract::new(admin);

    //         // Transfer event triggered during initial construction.
    //         let emitted_events = ink::env::test::recorded_events().collect::<Vec<_>>();
    //         assert_eq!(0, emitted_events.len());
    //     }

    //     #[ink::test]
    //     fn test_otp_generation() {
    //         let mut sts_contract = init_contract();
    //         let mut map = ink::prelude::collections::HashMap::new();

    //         for _i in 0..1000_000 {
    //             let otp = sts_contract.get_pseudo_random();
    //             if map.contains_key(&otp) {
    //                 let val = *map.get(&otp).unwrap();
    //                 map.insert(otp, val + 1);
    //             } else {
    //                 map.insert(otp, 1);
    //             }
    //             // debug_println!("{} OTP: {}", _i, otp);
    //         }

    //         for (k, v) in map.into_iter() {
    //             if v != 1 {
    //                 debug_println!("otp: {}, freq: {}", k, v);
    //             }
    //         }
    //     }

    //     #[ink::test]
    //     fn check_threshold() {
    //         // let alice: AccountId = default_accounts::<DefaultAccounts>().alice;
    //         // let mut sts_contract = PaymentContract::new(alice);
    //         let admin = AccountId::from([0x03; 32]);
    //         let mut sts_contract = PaymentContract::new(admin);

    //         let initial = sts_contract.view_threshold_value();
    //         assert_eq!(100, initial);

    //         set_caller::<DefaultEnvironment>(admin);
    //         let result = sts_contract.set_threshold_value(1000);
    //         assert_eq!(result, Ok(()));
    //         assert_eq!(1000, sts_contract.view_threshold_value());

    //         set_caller::<DefaultEnvironment>(AccountId::from([0x04; 32]));
    //         let result = sts_contract.set_threshold_value(100);
    //         assert_eq!(result, Err(Error::InvalidCaller));
    //     }

    //     #[test]
    //     fn test_creating_payment_record() {
    //         let receiver = AccountId::from([0x05; 32]);
    //         let sender = AccountId::from([0x04; 32]);
    //         let admin = AccountId::from([0x03; 32]);

    //         let mut sts_contract = init_contract();

    //         set_caller::<DefaultEnvironment>(admin);
    //         let threshold = sts_contract.set_threshold_value(1000);
    //         let amount = 100;
    //         let otp = sts_contract.get_pseudo_random();

    //         // let contract_id = ink::env::account_id::<DefaultEnvironment>();
    //         // transfer::<DefaultEnvironment>(contract_id, amount);

    //         set_caller::<DefaultEnvironment>(sender);
    //         set_callee::<DefaultEnvironment>(receiver);
    //         set_account_balance::<DefaultEnvironment>(sender, 5000);
    //         transfer_in::<DefaultEnvironment>(amount);

    //         let result = sts_contract.send_payment(receiver, amount);
    //         assert_eq!(result, Err(Error::BelowThresholdValue));

    //         set_caller::<DefaultEnvironment>(sender);
    //         set_callee::<DefaultEnvironment>(receiver);
    //         transfer_in::<DefaultEnvironment>(1000);
    //         let result = sts_contract.send_payment(receiver, 2000);
    //         assert_eq!(result, Err(Error::BalanceMismatch));

    //         set_caller::<DefaultEnvironment>(sender);
    //         set_callee::<DefaultEnvironment>(receiver);
    //         set_account_balance::<DefaultEnvironment>(sender, 5000);
    //         transfer_in::<DefaultEnvironment>(2000);
    //         set_block_timestamp::<DefaultEnvironment>(100);
    //         let result = sts_contract.send_payment(receiver, 2000);
    //         let result = sts_contract.send_payment(receiver, 2000);
    //         assert_eq!(result, Err(Error::TxnIDAlreadExists));

    //         set_caller::<DefaultEnvironment>(sender);
    //         set_callee::<DefaultEnvironment>(receiver);
    //         transfer_in::<DefaultEnvironment>(2000);
    //         set_block_timestamp::<DefaultEnvironment>(200);
    //         let result = sts_contract.send_payment(receiver, 2000);
    //         // assert_eq!(result, Ok());
    //         let events = recorded_events().collect::<Vec<_>>();
    //         assert_eq!(1, events.len());
    //     }

    //     #[test]
    //     fn test_otp_verify() {}

    //     fn init_contract() -> PaymentContract {
    //         set_caller::<DefaultEnvironment>(default_accounts::<DefaultEnvironment>().alice);
    //         let admin = AccountId::from([0x03; 32]);
    //         PaymentContract::new(admin)
    //     }
    // }



// #![cfg_attr(not(feature = "std"), no_std,no_main)]

// #[ink::contract]
// mod flipper {

//     /// Defines the storage of your contract.
//     /// Add new fields to the below struct in order
//     /// to add new static storage fields to your contract.
//     #[ink(storage)]
//     pub struct Flipper {
//         /// Stores a single `bool` value on the storage.
//         value: bool,
//     }

//     impl Flipper {
//         /// Constructor that initializes the `bool` value to the given `init_value`.
//         #[ink(constructor)]
//         pub fn new(init_value: bool) -> Self {
//             Self { value: init_value }
//         }

//         /// Constructor that initializes the `bool` value to `false`.
//         ///
//         /// Constructors can delegate to other constructors.
//         #[ink(constructor)]
//         pub fn default() -> Self {
//             Self::new(Default::default())
//         }

//         /// A message that can be called on instantiated contracts.
//         /// This one flips the value of the stored `bool` from `true`
//         /// to `false` and vice versa.
//         #[ink(message)]
//         pub fn flip(&mut self) {
//             self.value = !self.value;
//         }

//         /// Simply returns the current value of our `bool`.
//         #[ink(message)]
//         pub fn get(&self) -> bool {
//             self.value
//         }
//     }

//     /// Unit tests in Rust are normally defined within such a `#[cfg(test)]`
//     /// module and test functions are marked with a `#[test]` attribute.
//     /// The below code is technically just normal Rust code.
//     #[cfg(test)]
//     mod tests {
//         /// Imports all the definitions from the outer scope so we can use them here.
//         use super::*;

//         /// We test if the default constructor does its job.
//         #[ink::test]
//         fn default_works() {
//             let flipper = Flipper::default();
//             assert_eq!(flipper.get(), false);
//         }

//         /// We test a simple use case of our contract.
//         #[ink::test]
//         fn it_works() {
//             let mut flipper = Flipper::new(false);
//             assert_eq!(flipper.get(), false);
//             flipper.flip();
//             assert_eq!(flipper.get(), true);
//         }
//     }
// }

// #![cfg_attr(not(feature = "std"), no_std, no_main)]

// #[ink::contract]
// mod vesting_contract {

//     #[ink(storage)]
//     pub struct VestingContract {
//         releasable_balance: Balance,
//         released_balance: Balance,
//         duration_time: Timestamp,
//         start_time: Timestamp,
//         beneficiary: AccountId,
//         owner: AccountId,
//     }

//     /// Error for when the beneficiary is a zero address.
//     /// & Error for when the releasable balance is zero.
//     #[derive(Debug, PartialEq, Eq)]
//     #[ink::scale_derive(Encode, Decode, TypeInfo)]
//     pub enum Error {
//         InvalidBeneficiary,
//         ZeroReleasableBalance,
//     }

//     /// To emit events when a release is made.
//     #[ink(event)]
//     pub struct Released {
//         value: Balance,
//         to: AccountId,
//     }

//     /// ## This is to set the following during contract deployment:
//     /// - beneficiary: the account that will receive the tokens
//     /// - duration_time: duration of the vesting period,
//     ///   please note that this is in seconds
//     /// - start_time: the time (as Unix time) at which point
//     ///   vesting starts
//     /// - owner: the account that can release the tokens
//     /// - releasable_balance: the initial amount of tokens vested
//     /// - released_balance: the initial amount of tokens released
//     ///
//     /// # Note:
//     /// The beneficiary cannot be the zero address.
//     impl VestingContract {
//         #[ink(constructor, payable)]
//         pub fn new(
//             beneficiary: AccountId,
//             duration_time_in_sec: Timestamp,
//         ) -> Result<Self, Error> {
//             if beneficiary == AccountId::from([0x0; 32]) {
//                 return Err(Error::InvalidBeneficiary)
//             }

//             // This is multiplied by 1000 to conform to the
//             // Timestamp fomat in ink.
//             let duration_time = duration_time_in_sec.checked_mul(1000).unwrap();

//             let start_time = Self::env().block_timestamp();
//             let owner = Self::env().caller();
//             let releasable_balance = 0;
//             let released_balance = 0;

//             Ok(Self {
//                 duration_time,
//                 start_time,
//                 beneficiary,
//                 owner,
//                 releasable_balance,
//                 released_balance,
//             })
//         }

//         /// This returns current block timestamp.
//         pub fn time_now(&self) -> Timestamp {
//             self.env().block_timestamp()
//         }

//         /// This returns this contract balance.
//         #[ink(message)]
//         pub fn this_contract_balance(&self) -> Balance {
//             self.env().balance()
//         }

//         /// This returns the beneficiary wallet addr.
//         #[ink(message)]
//         pub fn beneficiary(&self) -> AccountId {
//             self.beneficiary
//         }

//         /// This returns the time at which point
//         /// vesting starts.
//         #[ink(message)]
//         pub fn start_time(&self) -> Timestamp {
//             self.start_time
//         }

//         /// This returns the duration of the vesting
//         /// period, in seconds.
//         #[ink(message)]
//         pub fn duration_time(&self) -> Timestamp {
//             self.duration_time
//         }

//         /// This returns the time at which point
//         /// vesting ends.
//         #[ink(message)]
//         pub fn end_time(&self) -> Timestamp {
//             self.start_time().checked_add(self.duration_time()).unwrap()
//         }

//         /// This returns the amount of time remaining
//         /// until the end of the vesting period.
//         #[ink(message)]
//         pub fn time_remaining(&self) -> Timestamp {
//             if self.time_now() < self.end_time() {
//                 self.end_time().checked_sub(self.time_now()).unwrap()
//             } else {
//                 0
//             }
//         }

//         /// This returns the amount of native token that
//         /// has already vested.
//         #[ink(message)]
//         pub fn released_balance(&self) -> Balance {
//             self.released_balance
//         }

//         /// This returns the amount of native token that
//         /// is currently available for release.
//         #[ink(message)]
//         pub fn releasable_balance(&self) -> Balance {
//             (self.vested_amount() as Balance)
//                 .checked_sub(self.released_balance())
//                 .unwrap()
//         }

//         /// This calculates the amount that has already vested
//         /// but hasn't been released from the contract yet.
//         #[ink(message)]
//         pub fn vested_amount(&self) -> Balance {
//             self.vesting_schedule(self.this_contract_balance(), self.time_now())
//         }

//         /// This sends the releasable balance to the beneficiary.
//         /// wallet address; no matter who triggers the release.
//         #[ink(message)]
//         pub fn release(&mut self) -> Result<(), Error> {
//             let releasable = self.releasable_balance();
//             if releasable == 0 {
//                 return Err(Error::ZeroReleasableBalance)
//             }

//             self.released_balance =
//                 self.released_balance.checked_add(releasable).unwrap();
//             self.env()
//                 .transfer(self.beneficiary, releasable)
//                 .expect("Transfer failed during release");

//             self.env().emit_event(Released {
//                 value: releasable,
//                 to: self.beneficiary,
//             });

//             Ok(())
//         }

//         /// This calculates the amount of tokens that have vested up
//         /// to the given current_time.
//         ///
//         /// The vesting schedule is linear, meaning tokens are
//         /// released evenly over the vesting duration.
//         ///
//         /// # Parameters:
//         /// - total_allocation: The total number of tokens
//         ///   allocated for vesting.
//         /// - current_time: The current timestamp for which
//         ///   we want to check the vested amount.
//         ///
//         /// # Returns:
//         /// - `0` if the current_time is before the vesting start time.
//         /// - total_allocation if the current_time is after the vesting
//         ///   end time or at least equal to it.
//         /// - A prorated amount based on how much time has passed since
//         ///   the start of the vesting period if the `current_time` is
//         ///   during the vesting period.
//         ///
//         /// # Example:
//         /// If the vesting duration is 200 seconds and 100 seconds have
//         /// passed since the start time, then 50% of the total_allocation
//         /// would have vested.
//         pub fn vesting_schedule(
//             &self,
//             total_allocation: Balance,
//             current_time: Timestamp,
//         ) -> Balance {
//             if current_time < self.start_time() {
//                 0
//             } else if current_time >= self.end_time() {
//                 return total_allocation
//             } else {
//                 return (total_allocation.checked_mul(
//                     (current_time.checked_sub(self.start_time()).unwrap()) as Balance,
//                 ))
//                 .unwrap()
//                 .checked_div(self.duration_time() as Balance)
//                 .unwrap()
//             }
//         }
//     }

//     #[cfg(test)]
//     mod tests {
//         use super::*;

//         /// Checking that the default constructor does its job.
//         #[ink::test]
//         fn new_creates_contract_with_correct_values() {
//             let contract =
//                 VestingContract::new(AccountId::from([0x01; 32]), 200).unwrap();

//             assert_eq!(contract.beneficiary(), AccountId::from([0x01; 32]));
//             assert_eq!(contract.duration_time(), 200 * 1000);
//             assert_eq!(contract.released_balance(), 0);
//             assert_eq!(contract.releasable_balance(), 0);
//         }

//         /// There should be some time remaining before the vesting period ends.
//         #[ink::test]
//         fn time_remaining_works() {
//             let contract =
//                 VestingContract::new(AccountId::from([0x01; 32]), 200).unwrap();
//             assert!(contract.time_remaining() > 0);
//         }

//         /// # Checking that tokens cannot be released before
//         /// the vesting period:
//         ///     - Trying to release tokens before the vesting period
//         ///       has ended, it will return an error.
//         ///     - The released_balance should remain 0 since no tokens
//         ///       were released.
//         #[ink::test]
//         fn release_before_vesting_period_fails() {
//             let mut contract =
//                 VestingContract::new(AccountId::from([0x01; 32]), 200).unwrap();

//             assert_eq!(contract.release(), Err(Error::ZeroReleasableBalance));
//             assert_eq!(contract.released_balance(), 0);
//         }

//         /// # Checking if tokens can be released after the vesting period:
//         ///     - Setting the duration_time to 0 to simulate the end of
//         ///       the vesting period.
//         ///     - And then simulate a deposit into the contract.
//         ///     - After releasing, the released_balance should match the
//         ///       amount we simulated as a deposit.
//         #[ink::test]
//         fn release_after_vesting_period_works() {
//             let mut contract =
//                 VestingContract::new(AccountId::from([0x01; 32]), 0).unwrap();
//             contract.releasable_balance += 1000000;

//             assert_eq!(contract.release(), Ok(()));
//             assert_eq!(contract.released_balance(), 1000000);
//         }

//         /// # Checking the vesting_schedule function for a specific behavior:
//         ///     - Given a total allocation and a current time halfway through
//         ///       the vesting period, the vested amount should be half of
//         ///       the total allocation.
//         #[ink::test]
//         fn vesting_schedule_works() {
//             let contract =
//                 VestingContract::new(AccountId::from([0x01; 32]), 200).unwrap();

//             assert_eq!(
//                 contract.vesting_schedule(1000, contract.start_time() + 100 * 1000),
//                 500
//             );
//         }
//     }
// }



// // #![cfg_attr(not(feature = "std"), no_std, no_main)]

// // pub use self::incrementer::{
// //     Incrementer,
// //     IncrementerRef,
// // };

// // #[ink::contract]
// // mod incrementer {
// //     #[ink(storage)]
// //     pub struct Incrementer {
// //         value: i32,
// //     }

// //     impl Incrementer {
// //         #[ink(constructor)]
// //         pub fn new(init_value: i32) -> Self {
// //             Self { value: init_value }
// //         }

// //         #[ink(constructor)]
// //         pub fn new_default() -> Self {
// //             Self::new(Default::default())
// //         }

// //         #[ink(message)]
// //         pub fn inc(&mut self, by: i32) {
// //             self.value = self.value.checked_add(by).unwrap();
// //         }

// //         #[ink(message)]
// //         pub fn get(&self) -> i32 {
// //             self.value
// //         }
// //     }

// //     #[cfg(test)]
// //     mod tests {
// //         use super::*;

// //         #[ink::test]
// //         fn default_works() {
// //             let contract = Incrementer::new_default();
// //             assert_eq!(contract.get(), 0);
// //         }

// //         #[ink::test]
// //         fn it_works() {
// //             let mut contract = Incrementer::new(42);
// //             assert_eq!(contract.get(), 42);
// //             contract.inc(5);
// //             assert_eq!(contract.get(), 47);
// //             contract.inc(-50);
// //             assert_eq!(contract.get(), -3);
// //         }
// //     }
// // }