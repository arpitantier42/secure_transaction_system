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
asdghsad