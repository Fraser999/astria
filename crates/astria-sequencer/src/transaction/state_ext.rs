use astria_core::{
    primitive::v1::{
        TransactionId,
        ADDRESS_LEN,
    },
    protocol::transaction::v1alpha1::SignedTransaction,
};
use cnidarium::{
    StateRead,
    StateWrite,
};

fn transaction_context() -> &'static str {
    "transaction/context"
}

#[derive(Clone, Copy)]
pub(crate) struct TransactionContext {
    pub(crate) block_hash: [u8; 32],
    pub(crate) address_bytes: [u8; ADDRESS_LEN],
    pub(crate) transaction_id: TransactionId,
    pub(crate) source_action_index: u64,
}

pub(crate) trait StateWriteExt: StateWrite {
    fn put_transaction_context(
        &mut self,
        block_hash: [u8; 32],
        signed_tx: &SignedTransaction,
    ) -> TransactionContext {
        let context = TransactionContext {
            block_hash,
            address_bytes: signed_tx.address_bytes(),
            transaction_id: signed_tx.id(),
            source_action_index: 0,
        };
        self.object_put(transaction_context(), context);
        context
    }

    fn delete_current_transaction_context(&mut self) {
        self.object_delete(transaction_context());
    }
}

pub(crate) trait StateReadExt: StateRead {
    fn get_transaction_context(&self) -> Option<TransactionContext> {
        self.object_get(transaction_context())
    }
}

impl<T: ?Sized + StateRead> StateReadExt for T {}
impl<T: StateWrite> StateWriteExt for T {}
