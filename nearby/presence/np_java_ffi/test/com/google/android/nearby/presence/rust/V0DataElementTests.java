/*
 * Copyright 2024 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package com.google.android.nearby.presence.rust;

import static com.google.common.truth.Truth.assertThat;

import com.google.android.nearby.presence.rust.credential.CredentialBook;
import com.google.android.nearby.presence.rust.credential.CredentialBook.NoMetadata;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

@RunWith(JUnit4.class)
public class V0DataElementTests {

  /** A test which verifies TxPower to be serializable and deserializable. */
  @Test
  public void txPower() throws Exception {
    NpAdv npAdv = new NpAdv();
    try (V0AdvertisementBuilder builder = npAdv.newPublicV0Builder()) {
      V0DataElement.TxPower txPower = new V0DataElement.TxPower(npAdv, 1);
      builder.addDataElement(txPower);
      byte[] adv = builder.build();
      assertThat(adv).isNotNull();

      CredentialBook<NoMetadata> book = npAdv.newEmptyCredentialBook();
      DeserializeResult<NoMetadata> result = npAdv.deserializeAdvertisement(adv, book);
      DeserializedV0Advertisement<NoMetadata> deserialized = result.getAsV0();
      V0DataElement de = deserialized.getDataElement(0);
      assertThat(de).isInstanceOf(V0DataElement.TxPower.class);
    }
  }

  /** A test which verifies PSM to be serializable and deserializable. */
  @Test
  public void psm() throws Exception {
    NpAdv npAdv = new NpAdv();
    try (V0AdvertisementBuilder builder = npAdv.newPublicV0Builder()) {
      V0DataElement.Psm psm = new V0DataElement.Psm(npAdv, 1);
      builder.addDataElement(psm);
      byte[] adv = builder.build();
      assertThat(adv).isNotNull();

      CredentialBook<NoMetadata> book = npAdv.newEmptyCredentialBook();
      DeserializeResult<NoMetadata> result = npAdv.deserializeAdvertisement(adv, book);
      DeserializedV0Advertisement<NoMetadata> deserialized = result.getAsV0();
      V0DataElement de = deserialized.getDataElement(0);
      assertThat(de).isInstanceOf(V0DataElement.Psm.class);
      assertThat(((V0DataElement.Psm) de).getPsm()).isEqualTo(1);
    }
  }
}
