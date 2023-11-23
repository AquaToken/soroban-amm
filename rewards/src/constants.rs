// todo: cfg(test) doesn't work for submodule. fix it
#[cfg(not(test))]
pub(crate) const PAGE_SIZE: u64 = 1000;

#[cfg(test)]
pub(crate) const PAGE_SIZE: u64 = 2;
