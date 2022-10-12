use crate::core::{ClassHash, StarknetTransactionHash};
use crate::rpc::v02::types::request::BroadcastedDeclareTransaction;
use crate::rpc::v02::RpcContext;
use crate::sequencer::error::SequencerError;
use crate::sequencer::request::add_transaction::ContractDefinition;
use crate::sequencer::ClientApi;

crate::rpc::error::generate_rpc_error_subset!(AddDeclareTransactionError: InvalidContractClass);

impl From<SequencerError> for AddDeclareTransactionError {
    fn from(e: SequencerError) -> Self {
        use crate::sequencer::error::StarknetErrorCode::InvalidProgram;
        match e {
            SequencerError::StarknetError(e) if e.code == InvalidProgram => {
                Self::InvalidContractClass
            }
            _ => Self::Internal(e.into()),
        }
    }
}

#[derive(serde::Deserialize, Debug, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum Transaction {
    #[serde(rename = "DECLARE")]
    Declare(BroadcastedDeclareTransaction),
}

#[derive(serde::Deserialize, Debug, PartialEq, Eq)]
pub struct AddDeclareTransactionInput {
    declare_transaction: Transaction,
}

#[derive(serde::Serialize, Debug, PartialEq, Eq)]
pub struct AddDeclareTransactionOutput {
    transaction_hash: StarknetTransactionHash,
    class_hash: ClassHash,
}

pub async fn add_declare_transaction(
    context: RpcContext,
    input: AddDeclareTransactionInput,
) -> Result<AddDeclareTransactionOutput, AddDeclareTransactionError> {
    let Transaction::Declare(tx) = input.declare_transaction;
    let contract_definition: ContractDefinition = tx
        .contract_class
        .try_into()
        .map_err(|e| anyhow::anyhow!("Failed to convert contract definition: {}", e))?;

    let response = context
        .sequencer
        .add_declare_transaction(
            tx.version,
            tx.max_fee,
            tx.signature,
            tx.nonce,
            contract_definition,
            tx.sender_address,
            None,
        )
        .await?;

    Ok(AddDeclareTransactionOutput {
        transaction_hash: response.transaction_hash,
        class_hash: response.class_hash,
    })
}

#[cfg(test)]
mod tests {
    use stark_hash::StarkHash;

    use crate::core::{ContractAddress, Fee, TransactionNonce, TransactionVersion};
    use crate::rpc::v02::types::request::BroadcastedDeclareTransaction;
    use crate::rpc::v02::types::ContractClass;
    use crate::starkhash;

    use super::*;

    lazy_static::lazy_static! {
        pub static ref CONTRACT_DEFINITION_JSON: Vec<u8> = {
            let compressed_json = include_bytes!("../../../../fixtures/contract_definition.json.zst");
            zstd::decode_all(std::io::Cursor::new(compressed_json)).unwrap()
        };

        pub static ref CONTRACT_CLASS: ContractClass = {
            ContractClass::from_definition_bytes(&*CONTRACT_DEFINITION_JSON).unwrap()
        };

        pub static ref CONTRACT_CLASS_JSON: String = {
            serde_json::to_string(&*CONTRACT_CLASS).unwrap()
        };
    }

    mod parsing {
        use super::*;

        fn test_declare_txn() -> Transaction {
            Transaction::Declare(BroadcastedDeclareTransaction {
                max_fee: Fee(web3::types::H128::from_low_u64_be(1)),
                version: TransactionVersion::ZERO,
                signature: vec![],
                nonce: TransactionNonce(StarkHash::ZERO),
                contract_class: CONTRACT_CLASS.clone(),
                sender_address: ContractAddress::new_or_panic(StarkHash::from_u64(1)),
            })
        }

        #[test]
        fn positional_args() {
            use jsonrpsee::types::Params;

            let positional = format!(
                r#"[
                    {{
                        "type": "DECLARE",
                        "version": "0x0",
                        "max_fee": "0x1",
                        "signature": [],
                        "nonce": "0x0",
                        "contract_class": {},
                        "sender_address": "0x1"
                    }}
                ]"#,
                CONTRACT_CLASS_JSON.clone()
            );
            let positional = Params::new(Some(&positional));

            let input = positional.parse::<AddDeclareTransactionInput>().unwrap();
            let expected = AddDeclareTransactionInput {
                declare_transaction: test_declare_txn(),
            };
            assert_eq!(input, expected);
        }

        #[test]
        fn named_args() {
            use jsonrpsee::types::Params;

            let named = format!(
                r#"{{
                    "declare_transaction": {{
                        "type": "DECLARE",
                        "version": "0x0",
                        "max_fee": "0x1",
                        "signature": [],
                        "nonce": "0x0",
                        "contract_class": {},
                        "sender_address": "0x1"
                    }}
                }}"#,
                CONTRACT_CLASS_JSON.clone()
            );
            let named = Params::new(Some(&named));

            let input = named.parse::<AddDeclareTransactionInput>().unwrap();
            let expected = AddDeclareTransactionInput {
                declare_transaction: test_declare_txn(),
            };
            assert_eq!(input, expected);
        }
    }

    #[test_log::test(tokio::test)]
    async fn invalid_contract_definition() {
        let context = RpcContext::for_tests();

        let invalid_contract_class = ContractClass {
            program: "".to_owned(),
            ..CONTRACT_CLASS.clone()
        };

        let declare_transaction = Transaction::Declare(BroadcastedDeclareTransaction {
            version: TransactionVersion::ZERO,
            max_fee: Fee(Default::default()),
            signature: vec![],
            nonce: TransactionNonce(Default::default()),
            contract_class: invalid_contract_class,
            sender_address: ContractAddress::new_or_panic(StarkHash::from_u64(1)),
        });

        let input = AddDeclareTransactionInput {
            declare_transaction,
        };
        let error = add_declare_transaction(context, input).await.unwrap_err();
        assert_matches::assert_matches!(error, AddDeclareTransactionError::InvalidContractClass);
    }

    #[test_log::test(tokio::test)]
    async fn successful_declare() {
        let context = RpcContext::for_tests();

        let declare_transaction = Transaction::Declare(BroadcastedDeclareTransaction {
            version: TransactionVersion::ZERO,
            max_fee: Fee(Default::default()),
            signature: vec![],
            nonce: TransactionNonce(Default::default()),
            contract_class: CONTRACT_CLASS.clone(),
            sender_address: ContractAddress::new_or_panic(StarkHash::from_u64(1)),
        });

        let input = AddDeclareTransactionInput {
            declare_transaction,
        };
        let result = add_declare_transaction(context, input).await.unwrap();
        assert_eq!(
            result,
            AddDeclareTransactionOutput {
                transaction_hash: StarknetTransactionHash(starkhash!(
                    "04b3791d16301be48268bfe1c0da7a9ad458847fd4666c98057d3940ef31775d"
                )),
                class_hash: ClassHash(starkhash!(
                    "050b2148c0d782914e0b12a1a32abe5e398930b7e914f82c65cb7afce0a0ab9b"
                )),
            }
        );
    }
}
