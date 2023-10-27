use core::hash::Hash;

use alloc::vec;
use alloc::vec::Vec;

use crate::buf::{Buf, Visit};
use crate::error::{Error, ErrorKind};
use crate::phf::hashing::{displace, hash, HashKey, Hashes};
use crate::phf::Entry;
use crate::{ByteOrder, Ref, Size, ZeroCopy};

use rand::distributions::Standard;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

const DEFAULT_LAMBDA: usize = 5;
const FIXED_SEED: u64 = 1234567890;

pub(crate) struct HashState {
    pub(crate) key: HashKey,
    pub(crate) map: Vec<usize>,
}

/// Calculate displacements length.
pub(crate) fn displacements_len(len: usize) -> usize {
    (len + DEFAULT_LAMBDA - 1) / DEFAULT_LAMBDA
}

pub(crate) fn generate_hash<K, T, F, E: ByteOrder, O: Size>(
    buf: &mut Buf,
    entries: &Ref<[T], E, O>,
    displacements: &Ref<[Entry<u32, u32>], E, O>,
    access: F,
) -> Result<HashState, Error>
where
    K: Visit,
    K::Target: Hash,
    F: Fn(&T) -> &K,
    T: ZeroCopy,
{
    for key in SmallRng::seed_from_u64(FIXED_SEED).sample_iter(Standard) {
        if let Some(hash) = try_generate_hash(buf, entries, displacements, key, &access)? {
            return Ok(hash);
        }
    }

    Err(Error::new(ErrorKind::FailedPhf))
}

fn try_generate_hash<K, T, F, E: ByteOrder, O: Size>(
    buf: &mut Buf,
    entries: &Ref<[T], E, O>,
    displacements: &Ref<[Entry<u32, u32>], E, O>,
    key: HashKey,
    access: &F,
) -> Result<Option<HashState>, Error>
where
    K: Visit,
    K::Target: Hash,
    F: ?Sized + Fn(&T) -> &K,
    T: ZeroCopy,
{
    let mut hashes = Vec::new();

    for n in 0..entries.len() {
        let Some(entry) = entries.get(n) else {
            return Err(Error::new(ErrorKind::IndexOutOfBounds {
                index: n,
                len: entries.len(),
            }));
        };

        let entry = buf.load(entry)?;
        let entry_key = access(entry);

        let h = hash(buf, entry_key, &key)?;
        hashes.push(h);
    }

    let mut buckets = vec![Vec::<usize>::new(); displacements.len()];

    for (index, hash) in hashes.iter().enumerate() {
        let to = hash.g % buckets.len();
        buckets[to].push(index);
    }

    buckets.sort_by_key(|a| a.len());

    let table_len = hashes.len();
    let mut map = vec![usize::MAX; table_len];

    // store whether an element from the bucket being placed is
    // located at a certain position, to allow for efficient overlap
    // checks. It works by storing the generation in each cell and
    // each new placement-attempt is a new generation, so you can tell
    // if this is legitimately full by checking that the generations
    // are equal. (A u64 is far too large to overflow in a reasonable
    // time for current hardware.)
    let mut try_map = vec![0u64; table_len];
    let mut generation = 0u64;

    // the actual values corresponding to the markers above, as
    // (index, key) pairs, for adding to the main map once we've
    // chosen the right displacements.
    let mut values_to_add = vec![];

    'outer: for (bucket, d_ref) in buckets.iter().zip(displacements.iter()) {
        for d1 in 0..(table_len as u32) {
            'inner: for d2 in 0..(table_len as u32) {
                values_to_add.clear();
                generation += 1;

                for &key in bucket {
                    let Hashes { f1, f2, .. } = hashes[key];
                    let index = displace(f1, f2, d1, d2) as usize;
                    let index = index % table_len;

                    if map[index] != usize::MAX || try_map[index] == generation {
                        continue 'inner;
                    }

                    try_map[index] = generation;
                    values_to_add.push((index, key));
                }

                // We've picked a good set of displacements
                *buf.load_mut(d_ref)? = Entry::new(d1, d2);

                for &(i, key) in &values_to_add {
                    map[i] = key;
                }

                continue 'outer;
            }
        }

        // Unable to find displacements for a bucket
        return Ok(None);
    }

    Ok(Some(HashState { key, map }))
}
