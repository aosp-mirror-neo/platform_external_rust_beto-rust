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

use jni::{
    descriptors::Desc,
    objects::{AutoLocal, JClass, JObject, JObjectArray},
    JNIEnv,
};

/// Collects an iterator of `I`, calling `item_into_jobject` on each element and collecting them
/// into a `JObjectArray`.
pub(crate) fn collect_to_jarray<'local, I, O>(
    env: &mut JNIEnv<'local>,
    element_class: impl Desc<'local, JClass<'local>>,
    iter: impl ExactSizeIterator<Item = I>,
    item_into_jobject: impl Fn(
        &mut JNIEnv<'local>,
        I,
    ) -> Result<AutoLocal<'local, O>, jni::errors::Error>,
) -> Result<JObjectArray<'local>, jni::errors::Error>
where
    O: Into<JObject<'local>> + AsRef<JObject<'local>>,
{
    let arr = env.new_object_array(
        iter.len()
            .try_into()
            .expect("Array length should be convertible to i32"),
        element_class,
        JObject::default(),
    )?;
    for (i, item) in iter.enumerate() {
        let item = item_into_jobject(env, item)?;
        env.set_object_array_element(
            &arr,
            i.try_into()
                .expect("Array index should be convertible to i32"),
            item,
        )?;
    }
    Ok(arr)
}

/// Define a handle suitable for JNI use.
///
/// In addition to declaring [handle_map::declare_handle_map!], this also generates conversions to
/// and from `jlong` for use with JNI. The handle can be converted to `jlong` using
/// `fooHandle.jlong()`. A `jlong` value can be converted back to `handle` using [`TryFrom`].
///
/// # Example
/// ```no_run
/// declare_handle! {
///   pub struct FooHandle for Foo;
/// }
/// ```
macro_rules! declare_handle {
    ( pub struct $handlename:ident for $target:ty; ) => {
        #[repr(C)]
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub struct $handlename {
            pub(crate) handle_id: u64,
        }

        impl $handlename {
            pub fn jlong(&self) -> jlong {
                self.handle_id
                    .try_into()
                    .expect("Handle ID can't be larger than max_active_handles(131072)")
            }
        }

        ::handle_map::declare_handle_map!(
            ::handle_map::HandleMapDimensions {
                num_shards: 4,
                max_active_handles: 131072
            },
            $handlename,
            $target
        );

        impl TryFrom<jlong> for $handlename {
            type Error = ::handle_map::HandleNotPresentError;

            fn try_from(value: jlong) -> Result<Self, Self::Error> {
                Ok(Self {
                    handle_id: u64::try_from(value)
                        .map_err(|_| ::handle_map::HandleNotPresentError)?,
                })
            }
        }
    };
}
pub(crate) use declare_handle;
