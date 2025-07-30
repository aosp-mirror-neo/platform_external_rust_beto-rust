// Copyright 2024 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use core::ops::{Deref, DerefMut};

/// An RAII read lock guard which can point to a subfield of the protected data.
pub struct MappedGuard<G: Deref, M: RwMapping<Arg = G::Target>> {
    guard: G,
    mapping: M,
}

impl<G: Deref, M: RwMapping<Arg = G::Target>> MappedGuard<G, M> {
    /// Create a new MappedGuard from a guard and a mapping.
    pub fn new(guard: G, mapping: M) -> Self {
        Self { guard, mapping }
    }

    /// Un-map this guard returning the bare guard.
    pub fn unmap(self) -> G {
        self.guard
    }

    /// Apply another mapping to the guard. This returns an instance that applies the current
    /// mapping and then the given mapping.
    pub fn map<R>(
        self,
        m: impl RwMapping<Arg = M::Ret, Ret = R>,
    ) -> MappedGuard<G, impl RwMapping<Arg = G::Target, Ret = R>>
    where
        G::Target: Sized,
        M::Ret: 'static,
    {
        MappedGuard {
            guard: self.guard,
            mapping: compose(self.mapping, m),
        }
    }
}

impl<G: Deref, M: RwMapping<Arg = G::Target>> Deref for MappedGuard<G, M> {
    type Target = M::Ret;

    fn deref(&self) -> &Self::Target {
        self.mapping.map(&*self.guard)
    }
}

impl<G: DerefMut, M: RwMapping<Arg = G::Target>> DerefMut for MappedGuard<G, M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.mapping.map_mut(&mut *self.guard)
    }
}

/// Mapping functions which define how to map from one locked data type to a component of that locked data
pub trait RwMapping {
    /// The original locked data type
    type Arg;
    /// The returned mapped locked data type which is a component of the original locked data
    type Ret;
    /// Maps from Arg into Ret
    fn map<'a>(&self, arg: &'a Self::Arg) -> &'a Self::Ret;
    /// Mutably maps from Arg into Ret
    fn map_mut<'a>(&self, arg: &'a mut Self::Arg) -> &'a mut Self::Ret;
}

/// Compose two RwMapping instances into a new RwMapping
fn compose<T, U, R>(
    a: impl RwMapping<Arg = T, Ret = U>,
    b: impl RwMapping<Arg = U, Ret = R>,
) -> impl RwMapping<Arg = T, Ret = R>
where
    U: 'static,
{
    struct Composed<A, B>(A, B);

    impl<T, U, R, A, B> RwMapping for Composed<A, B>
    where
        A: RwMapping<Arg = T, Ret = U>,
        B: RwMapping<Arg = U, Ret = R>,
        U: 'static,
    {
        type Arg = T;
        type Ret = R;

        fn map<'a>(&self, arg: &'a Self::Arg) -> &'a Self::Ret {
            let u = self.0.map(arg);
            let r = self.1.map(u);
            r
        }

        fn map_mut<'a>(&self, arg: &'a mut Self::Arg) -> &'a mut Self::Ret {
            let u = self.0.map_mut(arg);
            let r = self.1.map_mut(u);
            r
        }
    }

    Composed(a, b)
}
