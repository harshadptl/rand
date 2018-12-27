// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Thread-local random number generator

use std::cell::UnsafeCell;

use {RngCore, CryptoRng, SeedableRng, Error};
use rngs::adapter::ReseedingRng;
use rngs::EntropyRng;
use rand_hc::Hc128Core;

// Rationale for using `UnsafeCell` in `ThreadRng`:
//
// Previously we used a `RefCell`, with an overhead of ~15%. There will only
// ever be one mutable reference to the interior of the `UnsafeCell`, because
// we only have such a reference inside `next_u32`, `next_u64`, etc. Within a
// single thread (which is the definition of `ThreadRng`), there will only ever
// be one of these methods active at a time.
//
// A possible scenario where there could be multiple mutable references is if
// `ThreadRng` is used inside `next_u32` and co. But the implementation is
// completely under our control. We just have to ensure none of them use
// `ThreadRng` internally, which is nonsensical anyway. We should also never run
// `ThreadRng` in destructors of its implementation, which is also nonsensical.
//
// The additional `Rc` is not strictly neccesary, and could be removed. For now
// it ensures `ThreadRng` stays `!Send` and `!Sync`, and implements `Clone`.


// Number of generated bytes after which to reseed `TreadRng`.
//
// The time it takes to reseed HC-128 is roughly equivalent to generating 7 KiB.
// We pick a treshold here that is large enough to not reduce the average
// performance too much, but also small enough to not make reseeding something
// that basically never happens.
const THREAD_RNG_RESEED_THRESHOLD: u64 = 32*1024*1024; // 32 MiB

/// The type returned by [`thread_rng`], essentially just a reference to the
/// PRNG in thread-local memory.
///
/// `ThreadRng` uses [`ReseedingRng`] wrapping the same PRNG as [`StdRng`],
/// which is reseeded after generating 32 MiB of random data. A single instance
/// is cached per thread and the returned `ThreadRng` is a reference to this
/// instance — hence `ThreadRng` is neither `Send` nor `Sync` but is safe to use
/// within a single thread. This RNG is seeded and reseeded via [`EntropyRng`]
/// as required.
///
/// Note that the reseeding is done as an extra precaution against entropy
/// leaks and is in theory unnecessary — to predict `ThreadRng`'s output, an
/// attacker would have to either determine most of the RNG's seed or internal
/// state, or crack the algorithm used.
///
/// Like [`StdRng`], `ThreadRng` is a cryptographically secure PRNG. The current
/// algorithm used is [HC-128], which is an array-based PRNG that trades memory
/// usage for better performance. This makes it similar to ISAAC, the algorithm
/// used in `ThreadRng` before rand 0.5.
///
/// Cloning this handle just produces a new reference to the same thread-local
/// generator.
/// 
/// [`thread_rng`]: ../fn.thread_rng.html
/// [`ReseedingRng`]: adapter/struct.ReseedingRng.html
/// [`StdRng`]: struct.StdRng.html
/// [`EntropyRng`]: struct.EntropyRng.html
/// [HC-128]: ../../rand_hc/struct.Hc128Rng.html
#[derive(Clone, Debug)]
pub struct ThreadRng {
    // use of raw pointer implies type is neither Send nor Sync
    rng: *mut ReseedingRng<Hc128Core, EntropyRng>,
}

thread_local!(
    static THREAD_RNG_KEY: UnsafeCell<ReseedingRng<Hc128Core, EntropyRng>> = {
        let mut entropy_source = EntropyRng::new();
        let r = Hc128Core::from_rng(&mut entropy_source).unwrap_or_else(|err|
                panic!("could not initialize thread_rng: {}", err));
        let rng = ReseedingRng::new(r,
                                    THREAD_RNG_RESEED_THRESHOLD,
                                    entropy_source);
        UnsafeCell::new(rng)
    }
);

/// Retrieve the lazily-initialized thread-local random number generator,
/// seeded by the system. Intended to be used in method chaining style,
/// e.g. `thread_rng().gen::<i32>()`, or cached locally, e.g.
/// `let mut rng = thread_rng();`.  Invoked by the `Default` trait, making
/// `ThreadRng::default()` equivelent.
///
/// For more information see [`ThreadRng`].
///
/// [`ThreadRng`]: rngs/struct.ThreadRng.html
pub fn thread_rng() -> ThreadRng {
    ThreadRng { rng: THREAD_RNG_KEY.with(|t| t.get()) }
}

impl Default for ThreadRng {
    fn default() -> ThreadRng {
        ::prelude::thread_rng()
    }
}

impl RngCore for ThreadRng {
    #[inline(always)]
    fn next_u32(&mut self) -> u32 {
        unsafe { (*self.rng).next_u32() }
    }

    #[inline(always)]
    fn next_u64(&mut self) -> u64 {
        unsafe { (*self.rng).next_u64() }
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        unsafe { (*self.rng).fill_bytes(dest) }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        unsafe { (*self.rng).try_fill_bytes(dest) }
    }
}

impl CryptoRng for ThreadRng {}


#[cfg(test)]
mod test {
    #[test]
    #[cfg(not(feature="stdweb"))]
    fn test_thread_rng() {
        use Rng;
        let mut r = ::thread_rng();
        r.gen::<i32>();
        assert_eq!(r.gen_range(0, 1), 0);
    }
}
