use std::sync::atomic::{AtomicPtr, Ordering};

pub(crate) struct MaybeTagged<T>(AtomicPtr<T>);

impl<T> MaybeTagged<T> {
    pub(crate) fn load_ptr(&self) -> *mut T {
        self.load_decomposed().0
    }
    pub(crate) fn load_decomposed(&self) -> (*mut T, usize) {
        let raw = self.0.load(Ordering::Acquire);
        Self::decompose_raw(raw)
    }

    #[inline]
    fn decompose_raw(raw: *mut T) -> (*mut T, usize) {
        (
            usize_to_ptr_with_provenance(
                raw as usize & !unused_bits::<T>(),
                raw,
            ),
            raw as usize & unused_bits::<T>(),
        )
    }

    pub(crate) fn store_composed(&self, ptr: *mut T, tag: usize) {
        let tagged = Self::compose_raw(ptr, tag);

        self.0.store(tagged, Ordering::Release);
    }

    #[inline]
    fn compose_raw(ptr: *mut T, tag: usize) -> *mut T {
        usize_to_ptr_with_provenance(
            (ptr as usize & !unused_bits::<T>()) | (tag & unused_bits::<T>()),
            ptr,
        )
    }

    pub(crate) fn store_ptr(&self, ptr: *mut T) {
        self.store_composed(ptr, 0);
    }

    pub(crate) fn compare_exchange(
        &self,
        expected: *mut T,
        new: *mut T,
        succes: Ordering,
        failure: Ordering,
    ) -> Result<(*mut T, usize), (*mut T, usize)> {
        self.compare_exchange_with_tag(expected, 0, new, 0, succes, failure)
    }

    pub(crate) fn compare_exchange_with_tag(
        &self,
        expected: *mut T,
        e_tag: usize,
        new: *mut T,
        n_tag: usize,
        succes: Ordering,
        failure: Ordering,
    ) -> Result<(*mut T, usize), (*mut T, usize)> {
        match self.0.compare_exchange(
            Self::compose_raw(expected, e_tag),
            Self::compose_raw(new, n_tag),
            succes,
            failure,
        ) {
            Ok(new) => Ok(Self::decompose_raw(new)),
            Err(other) => Err(Self::decompose_raw(other)),
        }
    }

    pub(crate) fn compare_exchange_tag(
        &self,
        e_tag: usize,
        tag: usize,
        succes: Ordering,
        failure: Ordering,
    ) -> Result<usize, usize> {
        let mut ptr = self.load_ptr();
        while let Err((other_ptr, other_tag)) = self
            .compare_exchange_with_tag(ptr, e_tag, ptr, tag, succes, failure)
        {
            if other_tag != e_tag {
                return Err(other_tag);
            }

            ptr = other_ptr;
        }

        Ok(tag)
    }

    pub(crate) fn load_tag(&self) -> usize {
        self.load_decomposed().1
    }

    pub(crate) fn as_std(&self) -> &AtomicPtr<T> {
        &self.0
    }
}

const fn align<T>() -> usize {
    core::mem::align_of::<T>()
}

const fn unused_bits<T>() -> usize {
    (1 << align::<T>().trailing_zeros()) - 1
}

fn usize_to_ptr_with_provenance<T>(addr: usize, prov: *mut T) -> *mut T {
    let ptr = prov.cast::<u8>();
    ptr.wrapping_add(addr.wrapping_sub(ptr as usize)).cast()
}
