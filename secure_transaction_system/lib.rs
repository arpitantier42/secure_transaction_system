#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod payment_contract {
    use ink::env::hash;
    use ink::prelude::string::{String, ToString};
    use ink::prelude::vec::Vec;
    use ink::primitives::AccountId as OtherAccountId;
    use ink::{
        env::{
            block_timestamp, debug_println,
            hash::{HashOutput, Sha2x256},
            DefaultEnvironment,
        },
        storage::Mapping,
    };

    const ATTEMPTS_LIMIT: u8 = 3;

    #[ink(storage)]
    pub struct PaymentContract {
        payment_records: Mapping<Hash, PaymentInfo>,
        threshold_value: u128,
        admin: AccountId,
        expiry_time: Timestamp,
        salt: u64,
    }

    // --------------------------- Custom Struct---------------------------

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
        txn_id: Hash,
    }

    #[ink(event)]
    pub struct SecurePaymentInfo {
        #[ink(topic)]
        sender: AccountId,
        #[ink(topic)]
        receiver: AccountId,
        amount: Balance,
        txn_id: Hash,
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
        PaymentNotExpired,
        // RESET not typed correctly,
        IncorrectREFUND,
        // Value overflowed
        Overflow,
        // Admin only operation
        InvalidCaller,
        // Below Threshold value
        BelowThresholdValue,
        // Not in waiting period
        OperationNotAllowed,
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
                threshold_value: u128::pow(10, 2),
                admin,
                expiry_time: 86_400,
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

        #[ink(message)]
        pub fn view_payment_record(&self, txn_id: Hash) -> PaymentInfo {
            let payment_info = self.payment_records.get(txn_id).unwrap();
            ink::env::debug_println!("{:?}", payment_info);
            payment_info
        }

        #[ink(message)]
        pub fn view_threshold_value(&self) -> u128 {
            let value = self.threshold_value;
            ink::env::debug_println!("{:?}", value);
            value
        }

        #[ink(message)]
        pub fn set_threshold_value(&mut self, threshold_value: Balance) -> Result<()> {
            if self.admin == self.env().caller() {
                self.threshold_value = threshold_value;
                Ok(())
            } else {
                Err(Error::InvalidCaller)
            }
        }

        #[ink(message)]
        pub fn view_expiry_time(&self) -> Timestamp {
            let time = self.expiry_time;
            ink::env::debug_println!("{:?}", time);
            time
        }

        #[ink(message)]
        pub fn set_expiry_time(&mut self, time: Timestamp) -> Result<()> {
            if self.admin == self.env().caller() {
                self.expiry_time = time;
                Ok(())
            } else {
                Err(Error::InvalidCaller)
            }
        }

        #[ink(message)]
        pub fn view_current_time(&self) -> Timestamp {
            let time = self.env().block_timestamp();
            ink::env::debug_println!("{:?}", time);
            time
        }

        #[ink(message)]
        pub fn view_otp(&self,txn_id: Hash) -> u32 {
            let payment_info = self.payment_records.get(txn_id).unwrap();
            let otp=payment_info.otp;
            ink::env::debug_println!("{:?}", otp);
            otp
        }

        // generate pesudo random 9 digit OTP
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

        /// Handles payment_info from sender
        #[ink(message, payable)]
        pub fn record_payment(&mut self, receiver: AccountId, amount: Balance) -> Result<()> {
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
            let denomination = u128::pow(10, 12);
            let amount_units = amount.checked_div(denomination);
            if amount_units.is_none() {
                return Err(Error::Overflow);
            }

            // Check if amount exceeds the threshold value
            let amount_units = amount_units.unwrap();
            if amount_units < self.threshold_value {
                return Err(Error::BelowThresholdValue);
            }

            // create fixed length random OTP (6 digits)
            let otp: u32 = self.get_pseudo_random();

            // Get payment_info and transaction_id
            let payment_info = self.create_payment_info(receiver, caller, amount, otp);

            // let transaction_id = self.get_transaction_id(&payment_info);
            let transaction_id = self.get_transaction_id(&payment_info);

            // DEBUG
            // debug_println!("Randomly generated OTP {:?}", &otp);
            debug_println!("Transaction ID {:?}", &transaction_id);
            debug_println!("Payment Info {:?}", &payment_info);
            debug_println!("Threshold value: {:?}", self.threshold_value);
            debug_println!("Units sents: {:?}", amount_units);

            // Insert the payment record
            if self
                .payment_records
                .insert(transaction_id, &payment_info)
                .is_some()
            {
                return Err(Error::TxnIDAlreadExists);
            } else {
                // Emit event for payment record request
                self.env().emit_event(SecurePaymentRequested {
                    sender: caller,
                    receiver,
                    amount,
                    txn_id: transaction_id,
                });
            }
            Ok(())
        }

        #[ink(message)]
        pub fn get_refund(&mut self, txn_id: Hash) -> Result<()> {
            let payment_info = self.payment_records.get(txn_id);
            if payment_info.is_none() {
                return Err(Error::PaymentRecordMissing);
            }
            let mut payment_info = payment_info.unwrap();

            let caller = self.env().caller();
            if caller != payment_info.sender {
                return Err(Error::InvalidSender);
            }

            if self.is_expired(payment_info.recorded_time) {
                // check sender has sent me the amount & receiver hasn't received it
                // else return err that no amount to be refunded
                payment_info.status = PaymentStatus::Refunded;
                self.env().emit_event(SecurePaymentInfo {
                    sender: payment_info.sender,
                    receiver: payment_info.receiver,
                    amount: payment_info.amount,
                    txn_id,
                    status: payment_info.status,
                });
                Ok(())
            } else {
                Err(Error::PaymentNotExpired)
            }
      
        }
             
        /// Handles txn_id & OTP from receiver for verification
        #[ink(message)]
        pub fn receive_payment(&mut self, txn_id: Hash, sent_otp: u32) -> Result<()> {
            // Get payment_info using txn_id
            let payment_info = self.payment_records.get(txn_id);
            if payment_info.is_none() {
                return Err(Error::PaymentRecordMissing);
            }
            let mut payment_info = payment_info.unwrap();

            let status = payment_info.status.clone();
            if status != PaymentStatus::Waiting
            {
                return Err(Error::OperationNotAllowed);
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
                    txn_id,
                    status: payment_info.status,
                });
                return Err(Error::TimeLimitExceeded);
            }

            // match the otps
            if payment_info.otp != sent_otp {
                use crate::payment_contract::ATTEMPTS_LIMIT;

                // if attempts exceeded the decided limit
                if payment_info.otp_attempts > ATTEMPTS_LIMIT {
                    self.all_attempts_done(&mut payment_info, txn_id)
                } else {
                    self.one_attempt_done(&mut payment_info, txn_id)
                }
            } else {
                // transfer amount to receiver
                self.env()
                    .transfer(payment_info.receiver, payment_info.amount)
                    .unwrap();

                // change status to success
                payment_info.status = PaymentStatus::Success;

                // emit success event
                self.env().emit_event(SecurePaymentInfo {
                    sender: payment_info.sender,
                    receiver: payment_info.receiver,
                    amount: payment_info.amount,
                    txn_id,
                    status: payment_info.status.clone(),
                });
                Ok(())
            }
        }

        fn all_attempts_done(&self, payment_info: &mut PaymentInfo, txn_id: Hash) -> Result<()> {
            // refund payment to sender
            self.env()
                .transfer(payment_info.sender, payment_info.amount)
                .unwrap();
            self.payment_records.remove(txn_id);

            payment_info.status = PaymentStatus::AllAttemptsFailed;

            self.env().emit_event(SecurePaymentInfo {
                sender: payment_info.sender,
                receiver: payment_info.receiver,
                amount: payment_info.amount,
                txn_id,
                status: payment_info.status.clone(),
            });
            Err(Error::AttemptsExceedLimit)
        }

        fn one_attempt_done(&self, payment_info: &mut PaymentInfo, txn_id: Hash) -> Result<()> {
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
                txn_id,
                status: payment_info.status.clone(),
            });
            Err(Error::WrongOTP)
        }

        fn is_expired(&self, recorded_time: u64) -> bool {
            // 1 day has 86,400 seconds
            let one_day = 86_400;
            block_timestamp::<DefaultEnvironment>() > recorded_time.checked_add(one_day).unwrap()
        }

        // #[ink(message)]
        // pub fn get_account_id(&self) -> AccountId{
        //     self.env().account_id()
        // }

        // Returns the Hash of otp
        // fn encrypt_otp(&self, otp: u32) -> Hash {
        //     let mut encrypted_otp = <Sha2x256 as HashOutput>::Type::default();
        //     ink::env::hash_encoded::<Sha2x256, _>(&otp, &mut encrypted_otp);
        //     Hash::from(encrypted_otp)
        // }

        /// Returns the txn_id of payment_info
        fn get_transaction_id(&self, payment_info: &PaymentInfo) -> Hash {
            let encodable = payment_info;
            let mut txn_id = <Sha2x256 as HashOutput>::Type::default();
            ink::env::hash_encoded::<Sha2x256, _>(&encodable, &mut txn_id);
            Hash::from(txn_id)
        }
    }

    #[cfg(test)]
    mod tests {
        use ink::env::{
            debug_print,
            test::{self, recorded_events, set_block_timestamp, transfer_in},
            DefaultEnvironment,
        };

        use super::*;
        use ink::{
            env::{
                test::{
                    default_accounts, set_account_balance, set_callee, set_caller, DefaultAccounts,
                },
                transfer,
            },
            primitives::AccountId,
        };

        #[ink::test]
        fn new_works() {
            let admin = AccountId::from([0x03; 32]);
            let sts_contract = PaymentContract::new(admin);

            // Transfer event triggered during initial construction.
            let emitted_events = ink::env::test::recorded_events().collect::<Vec<_>>();
            assert_eq!(0, emitted_events.len());
        }

        #[ink::test]
        fn test_otp_generation() {
            let mut sts_contract = init_contract();
            let mut map = ink::prelude::collections::HashMap::new();

            for _i in 0..1000_000 {
                let otp = sts_contract.get_pseudo_random();
                if map.contains_key(&otp) {
                    let val = *map.get(&otp).unwrap();
                    map.insert(otp, val + 1);
                } else {
                    map.insert(otp, 1);
                }
                // debug_println!("{} OTP: {}", _i, otp);
            }

            for (k, v) in map.into_iter() {
                if v != 1 {
                    debug_println!("otp: {}, freq: {}", k, v);
                }
            }
        }

        #[ink::test]
        fn check_threshold() {
            // let alice: AccountId = default_accounts::<DefaultAccounts>().alice;
            // let mut sts_contract = PaymentContract::new(alice);
            let admin = AccountId::from([0x03; 32]);
            let mut sts_contract = PaymentContract::new(admin);

            let initial = sts_contract.view_threshold_value();
            assert_eq!(100, initial);

            set_caller::<DefaultEnvironment>(admin);
            let result = sts_contract.set_threshold_value(1000);
            assert_eq!(result, Ok(()));
            assert_eq!(1000, sts_contract.view_threshold_value());

            set_caller::<DefaultEnvironment>(AccountId::from([0x04; 32]));
            let result = sts_contract.set_threshold_value(100);
            assert_eq!(result, Err(Error::InvalidCaller));
        }

        #[test]
        fn test_creating_payment_record() {
            let receiver = AccountId::from([0x05; 32]);
            let sender = AccountId::from([0x04; 32]);
            let admin = AccountId::from([0x03; 32]);

            let mut sts_contract = init_contract();

            set_caller::<DefaultEnvironment>(admin);
            let threshold = sts_contract.set_threshold_value(1000);
            let amount = 100;
            let otp = sts_contract.get_pseudo_random();

            // let contract_id = ink::env::account_id::<DefaultEnvironment>();
            // transfer::<DefaultEnvironment>(contract_id, amount);

            set_caller::<DefaultEnvironment>(sender);
            set_callee::<DefaultEnvironment>(receiver);
            set_account_balance::<DefaultEnvironment>(sender, 5000);
            transfer_in::<DefaultEnvironment>(amount);

            let result = sts_contract.record_payment(receiver, amount);
            assert_eq!(result, Err(Error::BelowThresholdValue));

            set_caller::<DefaultEnvironment>(sender);
            set_callee::<DefaultEnvironment>(receiver);
            transfer_in::<DefaultEnvironment>(1000);
            let result = sts_contract.record_payment(receiver, 2000);
            assert_eq!(result, Err(Error::BalanceMismatch));

            set_caller::<DefaultEnvironment>(sender);
            set_callee::<DefaultEnvironment>(receiver);
            set_account_balance::<DefaultEnvironment>(sender, 5000);
            transfer_in::<DefaultEnvironment>(2000);
            set_block_timestamp::<DefaultEnvironment>(100);
            let result = sts_contract.record_payment(receiver, 2000);
            let result = sts_contract.record_payment(receiver, 2000);
            assert_eq!(result, Err(Error::TxnIDAlreadExists));

            set_caller::<DefaultEnvironment>(sender);
            set_callee::<DefaultEnvironment>(receiver);
            transfer_in::<DefaultEnvironment>(2000);
            set_block_timestamp::<DefaultEnvironment>(200);
            let result = sts_contract.record_payment(receiver, 2000);
            // assert_eq!(result, Ok());
            let events = recorded_events().collect::<Vec<_>>();
            assert_eq!(1, events.len());
        }

        #[test]
        fn test_otp_verify() {}

        fn init_contract() -> PaymentContract {
            set_caller::<DefaultEnvironment>(default_accounts::<DefaultEnvironment>().alice);
            let admin = AccountId::from([0x03; 32]);
            PaymentContract::new(admin)
        }
    }
}
