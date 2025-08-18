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

import java.util.Random;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

@RunWith(JUnit4.class)
public class V1DataElementTests {
  private static final int NUM_RANDOM_TRIALS = 1000;

  private final NpAdv npAdv = new NpAdv();

  private static final int TX_POWER_DE_TYPE = 5;
  private static final int CAST_ID_DE_TYPE = 17;
  private static final int DEDUP_HINT_DE_TYPE = 18;
  private static final int CONTEXT_SYNC_SEQ_NUM_DE_TYPE = 19;
  private static final int CONNECTIVITY_INFO_DE_TYPE = 20;
  private static final int DEVICE_TYPE_DE_TYPE = 22;
  private static final int MEDIA_DEDUPLICATION_ID_DE_TYPE = 23;

  /**
   * Checks that round-tripping a given DE through serialization and deserialization yields the same
   * data element.
   */
  private static void checkRoundtrip(V1DataElement dataElement) throws Exception {
    V1DataElement.Generic generic = dataElement.trySerialize();
    V1DataElement roundtripped = generic.tryDeserialize();

    assertThat(dataElement).isEqualTo(roundtripped);

    V1DataElement.Generic reconstructedGeneric = roundtripped.trySerialize();
    assertThat(generic).isEqualTo(reconstructedGeneric);
  }

  /**
   * A test which verifies that round-tripping Tx Powers through serialization and deserialization
   * is the identity.
   */
  @Test
  public void roundtripTxPower() throws Exception {
    for (int i = -100; i <= 20; i++) {
      checkRoundtrip(new V1DataElement.TxPower(npAdv, i));
    }
  }

  /**
   * A test which verifies that round-tripping Cast Ids through serialization and deserialization is
   * the identity.
   */
  @Test
  public void roundtripCastId() throws Exception {
    Random random = new Random(12345);
    byte[] id = new byte[32];

    for (int i = 0; i < NUM_RANDOM_TRIALS; i++) {
      random.nextBytes(id);
      checkRoundtrip(new V1DataElement.CastId(npAdv, id));
    }
  }

  /**
   * A test which verifies that round-tripping dedup hints through serialization and deserialization
   * is the identity.
   */
  @Test
  public void roundtripDedupHint() throws Exception {
    Random random = new Random(12345);
    byte[] hint = new byte[8];

    for (int i = 0; i < NUM_RANDOM_TRIALS; i++) {
      random.nextBytes(hint);
      checkRoundtrip(new V1DataElement.DedupHint(npAdv, hint));
    }
  }

  /**
   * A test which verifies that round-tripping context sync sequence numbers through serialization
   * and deserialization is the identity.
   */
  @Test
  public void roundtripContextSyncSeqNum() throws Exception {
    for (int i = 0; i <= 15; i++) {
      checkRoundtrip(new V1DataElement.ContextSyncSeqNum(npAdv, i));
    }
  }

  /**
   * A test which verifies that round-tripping device types through serialization and
   * deserialization is the identity.
   */
  @Test
  public void roundtripDeviceType() throws Exception {
    for (int i = 0; i <= 255; i++) {
      checkRoundtrip(new V1DataElement.DeviceType(npAdv, i));
    }
  }

  /**
   * A test which verifies that round-tripping connectivity info through serialization and
   * deserialization is the identity.
   */
  @Test
  public void roundtripDeviceInfo() throws Exception {
    Random random = new Random(12345);
    for (int i = 0; i < NUM_RANDOM_TRIALS; i++) {
      V1DataElement.ConnectivityInfo de = new V1DataElement.ConnectivityInfo(npAdv);
      if (random.nextBoolean()) {
        // Try to set BLE connectivity info.
        V1DataElement.BleConnectivityInfo bleInfo = new V1DataElement.BleConnectivityInfo();
        if (random.nextBoolean()) {
          byte[] macAddress = new byte[6];
          random.nextBytes(macAddress);
          V1DataElement.BleConnectivityInfo unused = bleInfo.setMacAddress(macAddress);
        }
        if (random.nextBoolean()) {
          // If we set a GATT service identifier, ensure that it's not
          // an empty array, since that will round-trip to `null` and
          // hence the round-trip comparison would fail.
          byte[] gattServiceIdentifier = new byte[random.nextInt(17) + 1];
          random.nextBytes(gattServiceIdentifier);
          V1DataElement.BleConnectivityInfo unused =
              bleInfo.setGattServiceIdentifier(gattServiceIdentifier);
        }
        if (random.nextBoolean()) {
          byte[] psm = new byte[2];
          random.nextBytes(psm);
          V1DataElement.BleConnectivityInfo unused = bleInfo.setPsm(psm);
        }
        if (random.nextBoolean()) {
          byte[] deviceToken = new byte[2];
          random.nextBytes(deviceToken);
          V1DataElement.BleConnectivityInfo unused = bleInfo.setDeviceToken(deviceToken);
        }
        // Ensure that the info is non-empty before we set it.
        if (!bleInfo.equals(new V1DataElement.BleConnectivityInfo())) {
          V1DataElement unused = de.setBleInfo(bleInfo);
        }
      }
      if (random.nextBoolean()) {
        // Try to set Wifi-LAN connectivity info.
        V1DataElement.WifiLanConnectivityInfo wifiLanInfo =
            new V1DataElement.WifiLanConnectivityInfo();
        if (random.nextBoolean()) {
          byte[] ipAddress;
          if (random.nextBoolean()) {
            ipAddress = new byte[4];
          } else {
            ipAddress = new byte[16];
          }
          random.nextBytes(ipAddress);
          V1DataElement.WifiLanConnectivityInfo unused = wifiLanInfo.setIpAddress(ipAddress);
        }
        if (random.nextBoolean()) {
          byte[] port = new byte[2];
          random.nextBytes(port);
          V1DataElement.WifiLanConnectivityInfo unused = wifiLanInfo.setPort(port);
        }
        if (random.nextBoolean()) {
          byte[] bssid = new byte[6];
          random.nextBytes(bssid);
          V1DataElement.WifiLanConnectivityInfo unused = wifiLanInfo.setBssid(bssid);
        }
        // Ensure that the info is non-empty before we set it.
        if (!wifiLanInfo.equals(new V1DataElement.WifiLanConnectivityInfo())) {
          V1DataElement unused = de.setWifiLanInfo(wifiLanInfo);
        }
      }
      // Empty info won't serialize, so skip if it's empty.
      if (de.equals(new V1DataElement.ConnectivityInfo(npAdv))) {
        continue;
      }
      checkRoundtrip(de);
    }
  }

  /**
   * A test which verifies that round-tripping Media Deduplication Ids through serialization and
   * deserialization is the identity.
   */
  @Test
  public void roundtripMediaDeduplicationId() throws Exception {
    Random random = new Random(12345);
    byte[] id = new byte[20];

    for (int i = 0; i < NUM_RANDOM_TRIALS; i++) {
      random.nextBytes(id);
      checkRoundtrip(new V1DataElement.MediaDeduplicationId(npAdv, id));
    }
  }

  /**
   * A test which verifies that round-tripping Capabilities through serialization and
   * deserialization is the identity.
   */
  @Test
  public void roundtripCapabilities() throws Exception {
    int maxNumSupportedCapabilities = 4;
    V1DataElement.Capabilities capabilities = new V1DataElement.Capabilities(npAdv);
    for (int i = 0; i < maxNumSupportedCapabilities; i++) {
      capabilities.addCapability(i);
      checkRoundtrip(capabilities);
    }
  }

  /**
   * A test which verifies that round-tripping Requirements through serialization and
   * deserialization is the identity.
   */
  @Test
  public void roundtripRequirements() throws Exception {
    int maxNumSupportedCapabilities = 4;
    V1DataElement.Requirements requirements = new V1DataElement.Requirements(npAdv);
    for (int i = 0; i < maxNumSupportedCapabilities; i++) {
      requirements.addCapability(i);
      checkRoundtrip(requirements);
    }
  }

  /**
   * A test which verifies that attempting to construct a generic DE with the forbidden type-code
   * 0xFFFFFFFF yields an exception.
   */
  @Test(expected = IllegalArgumentException.class)
  public void genericDataElementForbiddenTypeCodeIllegalArgumentException() throws Exception {
    byte[] payload = new byte[2];
    V1DataElement unused = new V1DataElement.Generic(npAdv, 0xFFFFFFFF, payload);
  }

  /**
   * A test which verifies that attempting to construct a generic DE with too long of a length
   * yields an exception.
   */
  @Test(expected = IllegalArgumentException.class)
  public void genericDataElementTooLongIllegalArgumentException() throws Exception {
    byte[] payload = new byte[128];
    V1DataElement unused = new V1DataElement.Generic(npAdv, 0, payload);
  }

  /**
   * A test which verifies that attempting to construct an out-of-bounds Tx Power yields an
   * exception.
   */
  @Test(expected = IllegalArgumentException.class)
  public void txPowerIllegalArgumentException() throws Exception {
    new V1DataElement.TxPower(npAdv, 120);
  }

  /**
   * A test which verifies that attempting to deserialize an out-of-bounds Tx Power yields an
   * exception.
   */
  @Test(expected = V1DataElement.TxPowerOutOfBoundsException.class)
  public void txPowerOutOfBoundsException() throws Exception {
    byte[] payload = {120};
    V1DataElement.Generic generic = new V1DataElement.Generic(npAdv, TX_POWER_DE_TYPE, payload);
    V1DataElement unused = generic.tryDeserialize();
  }

  /**
   * A test which verifies that attempting to deserialize a wrong-length Tx Power yields an
   * exception.
   */
  @Test(expected = V1DataElement.TxPowerWrongLengthException.class)
  public void txPowerWrongLengthException() throws Exception {
    byte[] payload = {0, 0};
    V1DataElement.Generic generic = new V1DataElement.Generic(npAdv, TX_POWER_DE_TYPE, payload);
    V1DataElement unused = generic.tryDeserialize();
  }

  /**
   * A test which verifies that attempting to construct a wrong-length Cast Id yields an exception.
   */
  @Test(expected = IllegalArgumentException.class)
  public void castIdIllegalArgumentException() throws Exception {
    byte[] payload = new byte[33];
    V1DataElement unused = new V1DataElement.CastId(npAdv, payload);
  }

  /**
   * A test which verifies that attempting to deserialize a wrong-length Cast Id yields an
   * exception.
   */
  @Test(expected = V1DataElement.CastIdWrongLengthException.class)
  public void castIdWrongLengthException() throws Exception {
    byte[] payload = new byte[33];
    V1DataElement.Generic generic = new V1DataElement.Generic(npAdv, CAST_ID_DE_TYPE, payload);
    V1DataElement unused = generic.tryDeserialize();
  }

  /**
   * A test which verifies that attempting to construct a wrong-length dedup hint yields an
   * exception.
   */
  @Test(expected = IllegalArgumentException.class)
  public void dedupHintIllegalArgumentException() throws Exception {
    byte[] payload = new byte[9];
    V1DataElement unused = new V1DataElement.DedupHint(npAdv, payload);
  }

  /**
   * A test which verifies that attempting to deserialize a wrong-length dedup hint yields an
   * exception.
   */
  @Test(expected = V1DataElement.DedupHintWrongLengthException.class)
  public void dedupHintWrongLengthException() throws Exception {
    byte[] payload = new byte[9];
    V1DataElement.Generic generic = new V1DataElement.Generic(npAdv, DEDUP_HINT_DE_TYPE, payload);
    V1DataElement unused = generic.tryDeserialize();
  }

  /**
   * A test which verifies that attempting to construct an out-of-bounds context sync sequence
   * number yields an exception.
   */
  @Test(expected = IllegalArgumentException.class)
  public void contextSyncSeqNumIllegalArgumentException() throws Exception {
    V1DataElement unused = new V1DataElement.ContextSyncSeqNum(npAdv, 16);
  }

  /**
   * A test which verifies that attempting to deserialize an out-of-bounds context sync sequence
   * number yields an exception.
   */
  @Test(expected = V1DataElement.ContextSyncSeqNumOutOfBoundsException.class)
  public void contextSyncSeqNumOutOfBoundsException() throws Exception {
    byte[] payload = {16};
    V1DataElement.Generic generic =
        new V1DataElement.Generic(npAdv, CONTEXT_SYNC_SEQ_NUM_DE_TYPE, payload);
    V1DataElement unused = generic.tryDeserialize();
  }

  /**
   * A test which verifies that attempting to deserialize a wrong-length context sync sequence
   * number yields an exception.
   */
  @Test(expected = V1DataElement.ContextSyncSeqNumWrongLengthException.class)
  public void contextSyncSeqNumWrongLengthException() throws Exception {
    byte[] payload = {0, 0};
    V1DataElement.Generic generic =
        new V1DataElement.Generic(npAdv, CONTEXT_SYNC_SEQ_NUM_DE_TYPE, payload);
    V1DataElement unused = generic.tryDeserialize();
  }

  /**
   * A test which verifies that attempting to construct an out-of-bounds device type yields an
   * exception.
   */
  @Test(expected = IllegalArgumentException.class)
  public void deviceTypeIllegalArgumentException() throws Exception {
    V1DataElement unused = new V1DataElement.DeviceType(npAdv, 256);
  }

  /**
   * A test which verifies that attempting to deserialize a wrong-length device type yields an
   * exception.
   */
  @Test(expected = V1DataElement.DeviceTypeWrongLengthException.class)
  public void deviceTypeWrongLengthException() throws Exception {
    byte[] payload = {0, 0};
    V1DataElement.Generic generic = new V1DataElement.Generic(npAdv, DEVICE_TYPE_DE_TYPE, payload);
    V1DataElement unused = generic.tryDeserialize();
  }

  /**
   * A test which verifies that attempting to serialize empty connectivity info yields an exception.
   */
  @Test(expected = IllegalStateException.class)
  public void emptyConnectivityInfoIllegalStateException() throws Exception {
    V1DataElement.ConnectivityInfo de = new V1DataElement.ConnectivityInfo(npAdv);
    V1DataElement.Generic unused = de.trySerialize();
  }

  /**
   * A test which verifies that attempting to deserialize empty connectivity info yields an
   * exception.
   */
  @Test(expected = V1DataElement.ConnectivityInfoMalformedException.class)
  public void emptyConnectivityInfoMalformedComponentsException() throws Exception {
    byte[] payload = {};
    V1DataElement.Generic generic =
        new V1DataElement.Generic(npAdv, CONNECTIVITY_INFO_DE_TYPE, payload);
    V1DataElement unused = generic.tryDeserialize();
  }

  /**
   * A test which verifies that attempting to deserialize invalid BLE connectivity info yields an
   * exception.
   */
  @Test(expected = V1DataElement.ConnectivityInfoComponentMalformedException.class)
  public void invalidBleInfoMalformedComponentException() throws Exception {
    // BLE component is specified, but the field mask is invalid (states that fields, even ones
    // which
    // aren't supposed to exist are present, but actually nothing follows.)
    byte[] payload = {(byte) 0x00, (byte) 0xFF};
    V1DataElement.Generic generic =
        new V1DataElement.Generic(npAdv, CONNECTIVITY_INFO_DE_TYPE, payload);
    V1DataElement unused = generic.tryDeserialize();
  }

  /** A test which verifies that attempting to assign a wrong-length BLE Mac yields an exception */
  @Test(expected = IllegalArgumentException.class)
  public void bleMacIllegalArgumentException() throws Exception {
    new V1DataElement.BleConnectivityInfo().setMacAddress(new byte[1]);
  }

  /**
   * A test which verifies that attempting to assign a wrong-length Gatt service identifier yields
   * an exception
   */
  @Test(expected = IllegalArgumentException.class)
  public void bleGattServiceIdentifierIllegalArgumentException() throws Exception {
    new V1DataElement.BleConnectivityInfo().setGattServiceIdentifier(new byte[18]);
  }

  /** A test which verifies that attempting to assign a wrong-length PSM yields an exception */
  @Test(expected = IllegalArgumentException.class)
  public void blePsmIllegalArgumentException() throws Exception {
    new V1DataElement.BleConnectivityInfo().setPsm(new byte[3]);
  }

  /**
   * A test which verifies that attempting to assign a wrong-length device token yields an exception
   */
  @Test(expected = IllegalArgumentException.class)
  public void bleDeviceTokenIllegalArgumentException() throws Exception {
    new V1DataElement.BleConnectivityInfo().setDeviceToken(new byte[4]);
  }

  /**
   * A test which verifies that attempting to assign a wrong-length IP address yields an exception
   */
  @Test(expected = IllegalArgumentException.class)
  public void wifiLanIpAddressIllegalArgumentException() throws Exception {
    new V1DataElement.WifiLanConnectivityInfo().setIpAddress(new byte[5]);
  }

  /** A test which verifies that attempting to assign a wrong-length port yields an exception */
  @Test(expected = IllegalArgumentException.class)
  public void wifiLanPortIllegalArgumentException() throws Exception {
    new V1DataElement.WifiLanConnectivityInfo().setPort(new byte[3]);
  }

  /** A test which verifies that attempting to assign a wrong-length BSSID yields an exception */
  @Test(expected = IllegalArgumentException.class)
  public void wifiLanBSSIDIllegalArgumentException() throws Exception {
    new V1DataElement.WifiLanConnectivityInfo().setBssid(new byte[7]);
  }

  /**
   * A test which verifies that attempting to construct a wrong-length Media Deduplication Id yields
   * an exception.
   */
  @Test(expected = IllegalArgumentException.class)
  public void mediaDeduplicationIdIllegalArgumentException() throws Exception {
    byte[] payload = new byte[21];
    V1DataElement unused = new V1DataElement.MediaDeduplicationId(npAdv, payload);
  }

  /**
   * A test which verifies that attempting to deserialize a wrong-length media deduplication Id
   * yields an exception.
   */
  @Test(expected = V1DataElement.MediaDeduplicationIdWrongLengthException.class)
  public void mediaDeduplicationIdWrongLengthException() throws Exception {
    byte[] payload = new byte[33];
    V1DataElement.Generic generic =
        new V1DataElement.Generic(npAdv, MEDIA_DEDUPLICATION_ID_DE_TYPE, payload);
    V1DataElement unused = generic.tryDeserialize();
  }

  /**
   * A test which verifies that attempting to construct a Capabilities DE with reserved bits throws
   * a CapabilitiesMalformedException.
   */
  @Test(expected = IndexOutOfBoundsException.class)
  public void capabilitiesMalformedException() throws Exception {
    V1DataElement.Capabilities capabilities = new V1DataElement.Capabilities(npAdv);
    for (int cap = 0; cap <= 7; cap++) {
      capabilities.addCapability(cap);
    }
    V1DataElement.Generic unused = capabilities.trySerialize();
  }

  /**
   * A test which verifies that attempting to construct a Requirements DE with reserved bits throws
   * a RequirementsMalformedException.
   */
  @Test(expected = IndexOutOfBoundsException.class)
  public void requirementsMalformedException() throws Exception {
    V1DataElement.Requirements requirements = new V1DataElement.Requirements(npAdv);
    for (int cap = 0; cap <= 7; cap++) {
      requirements.addCapability(cap);
    }
    V1DataElement.Generic unused = requirements.trySerialize();
  }
}
