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

use std::sync::Arc;

use jni::{
    objects::{GlobalRef, JObject},
    JNIEnv, JavaVM,
};
use log::error;
use pourover::desc::ClassDesc;
use submerge::{DeltaDocument, NetworkInterface};

use super::data_store::JavaDataStoreConfig;

#[derive(Debug, Clone)]
pub struct JavaNetwork {
    jobject: NetworkInterfaceJni<GlobalRef>,
    vm: Arc<JavaVM>,
}

impl JavaNetwork {
    pub fn new<'local>(
        env: &mut JNIEnv<'local>,
        network: NetworkInterfaceJni<JObject<'local>>,
    ) -> Result<Self, jni::errors::Error> {
        Ok(Self {
            jobject: network.to_global_ref(env)?,
            vm: Arc::new(env.get_java_vm()?),
        })
    }
}

impl NetworkInterface<JavaDataStoreConfig> for JavaNetwork {
    fn on_new_update(&mut self, doc_id: &str, document: DeltaDocument<JavaDataStoreConfig>) {
        let block = || -> Result<(), anyhow::Error> {
            let mut env = self.vm.get_env()?;
            self.jobject
                .on_new_update(&mut env, doc_id, &document.to_proto_bytes()?)?;
            Ok(())
        };
        match block() {
            Ok(_) => {}
            Err(err) => {
                error!("Error sending network update: {err}");
            }
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct NetworkInterfaceJni<Obj>(Obj);

static NETWORK_INTERFACE_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/submerge/NetworkInterface");

impl<'local, O: AsRef<JObject<'local>>> NetworkInterfaceJni<O> {
    pub fn to_global_ref(
        &self,
        env: &mut JNIEnv<'_>,
    ) -> Result<NetworkInterfaceJni<GlobalRef>, jni::errors::Error> {
        Ok(NetworkInterfaceJni(env.new_global_ref(&self.0)?))
    }

    pub fn on_new_update(
        &self,
        env: &mut JNIEnv<'_>,
        doc_id: &str,
        serialized_doc: &[u8],
    ) -> Result<(), jni::errors::Error> {
        let serialized_doc = env.auto_local(env.byte_array_from_slice(serialized_doc)?);
        pourover::call_method!(
            env,
            NETWORK_INTERFACE_CLASS,
            "onNewUpdate",
            "(Ljava/lang/String;[B)V",
            self.0.as_ref(),
            &env.new_string(doc_id)?,
            &serialized_doc,
        )?;
        Ok(())
    }
}
