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

import static java.lang.annotation.RetentionPolicy.SOURCE;

import androidx.annotation.IntDef;
import androidx.annotation.Nullable;
import java.lang.annotation.Retention;
import java.util.Arrays;
import java.util.BitSet;
import java.util.Objects;

/** Base class for V1 data element types. */
public abstract class V1DataElement {

  /**
   * A visitor interface that can be used while iterating data elements in an advertisement to avoid
   * checking {@code instanceof} on every one.
   */
  public interface Visitor {
    void visitGeneric(Generic generic);

    void visitTxPower(TxPower txPower);

    void visitCastId(CastId castId);

    void visitActions(Actions actions);

    void visitContextSyncSeqNum(ContextSyncSeqNum seqNum);

    void visitDedupHint(DedupHint dedupHint);

    void visitDeviceType(DeviceType deviceType);

    void visitConnectivityInfo(ConnectivityInfo connectivityInfo);
  }

  // All subclasses should be in this file
  private V1DataElement() {}

  /** Visit this advertisement with the given visitor. */
  public abstract void visit(Visitor v);

  /**
   * Serializes this data element to a generic DE.
   *
   * @throws IllegalStateException in cases where the data element which is being serialized has an
   *     invalid internal state, such as a Tx Power out of bounds or an Actions DE containing no
   *     actions (since none have been added yet). While most DEs have guards against this kind of
   *     exception by being immutable and having appropriate checking done in constructors, mutable,
   *     "unfinished" DEs may also throw this exception.
   * @throws TooMuchDataException in cases where the data element cannot be serialized due to
   *     running out of space in an ordinary V1 DE's payload (max 127 bytes.)
   */
  public abstract Generic trySerialize();

  /**
   * An exception raised when attempting to deserialize a DE from a `Generic` DE into a
   * representation for some more specific DE type.
   */
  @UsedByNative
  public static class DeserializationException extends Exception {
    protected DeserializationException(String message) {
      super(message);
    }
  }

  /**
   * An exception raised when attempting to serialize a DE, but the contents of the DE would wind up
   * being too large to fit within the up-to-127-byte payload of a V1 data element.
   */
  @UsedByNative
  public static class TooMuchDataException extends RuntimeException {
    protected TooMuchDataException() {
      super("DE has too much data to fit within a V1 DE");
    }
  }

  /**
   * A generic data element. This is a data element which has a type that is not known by this
   * library (e.g. a vendor-specific data element).
   */
  @UsedByNative
  public static final class Generic extends V1DataElement {
    private final long type;
    private final byte[] data;

    /**
     * Constructs a new generic DE with the given type-code and payload.
     *
     * @throws IllegalArgumentException if the DE payload contained more than 127 bytes.
     */
    public Generic(NpAdv npAdv, long type, byte[] data) {
      this(type, data);
      npAdv.ensureLoaded();
    }

    /**
     * Constructs a new generic DE with the given type-code and payload.
     *
     * @throws IllegalArgumentException if the DE payload contained more than 127 bytes.
     */
    @UsedByNative
    private Generic(long type, byte[] data) {
      this.type = type;
      this.data = Arrays.copyOf(data, data.length);
      if (type == 0xFFFFFFFF) {
        throw new IllegalArgumentException("Generic DE has a forbidden type-code!");
      }
      if (data.length > 127) {
        throw new IllegalArgumentException("Generic DE is too long!");
      }
    }

    @UsedByNative
    public long getType() {
      return type;
    }

    @UsedByNative
    public byte[] getData() {
      return Arrays.copyOf(data, data.length);
    }

    @Override
    public void visit(Visitor v) {
      v.visitGeneric(this);
    }

    /**
     * Attempts to deserialize the contents of this generic DE into a more specific data element
     * type.
     *
     * @return - If the DE type was unrecognized, the input will simply be returned unaltered. - If
     *     the DE type was recognized and the contents were valid for an instance of that DE type,
     *     returns an instance of the appropriate non-`Generic` inheriting subclass of
     *     `V1DataElement`.
     * @throws DeserializationException if the contents of the passed DE were invalid for the DE's
     *     type code.
     */
    public V1DataElement tryDeserialize() throws DeserializationException {
      return nativeTryDeserialize(this);
    }

    private static native V1DataElement nativeTryDeserialize(Generic generic)
        throws DeserializationException;

    @Override
    public Generic trySerialize() {
      return nativeTrySerialize(this);
    }

    private static native Generic nativeTrySerialize(Generic generic);

    @Override
    public boolean equals(@Nullable Object other) {
      if (other instanceof Generic) {
        Generic otherGeneric = (Generic) other;
        return this.type == otherGeneric.type && Arrays.equals(this.data, otherGeneric.data);
      }
      return false;
    }

    @Override
    public int hashCode() {
      return ((int) (31 * this.type)) + Arrays.hashCode(this.data);
    }

    @Override
    public String toString() {
      return "GenericDE { type_code: "
          + this.type
          + ", contents: "
          + formatByteArrayToString(this.data, ByteValuePrintFormat.HEX);
    }
  }

  /**
   * Exception raised upon attempting to deserialize a Tx Power which is out of the valid bounds
   * ([-20dBm-100dBm]) for a transmission power
   */
  @UsedByNative
  public static final class TxPowerOutOfBoundsException extends DeserializationException {
    @UsedByNative
    public TxPowerOutOfBoundsException() {
      super("Tx Power out of bounds");
    }
  }

  /**
   * Exception raised upon attempting to deserialize a Tx Power which fails to occupy exactly one
   * byte.
   */
  @UsedByNative
  public static final class TxPowerWrongLengthException extends DeserializationException {
    @UsedByNative
    public TxPowerWrongLengthException() {
      super("Tx Power payload doesn't occupy exactly one byte");
    }
  }

  /** A transmission power from -20dBm-100dBm. */
  @UsedByNative
  public static final class TxPower extends V1DataElement {
    @UsedByNative private final int txPower;

    /**
     * Constructs a new V1 Tx Power DE.
     *
     * @throws IllegalArgumentException if the transmission power was not in the allowable [-100,
     *     20] range.
     */
    public TxPower(NpAdv npAdv, int txPower) {
      this(txPower);
      npAdv.ensureLoaded();
    }

    @UsedByNative
    private TxPower(int txPower) {
      this.txPower = txPower;
      if (txPower > 20 || txPower < -100) {
        throw new IllegalArgumentException("Tx power is out of range");
      }
    }

    @UsedByNative
    public int getTxPower() {
      return txPower;
    }

    @Override
    public void visit(Visitor v) {
      v.visitTxPower(this);
    }

    @Override
    public Generic trySerialize() {
      return nativeTrySerialize(this);
    }

    private static native Generic nativeTrySerialize(TxPower txPower);

    @Override
    public boolean equals(@Nullable Object o) {
      if (o instanceof TxPower) {
        TxPower other = (TxPower) o;
        return other.txPower == this.txPower;
      }
      return false;
    }

    @Override
    public int hashCode() {
      return this.txPower;
    }

    @Override
    public String toString() {
      return "V1TxPower(" + this.txPower + ")";
    }
  }

  /**
   * Exception raised upon attempting to deserialize a Cast ID which fails to occupy exactly 32
   * bytes.
   */
  @UsedByNative
  public static final class CastIdWrongLengthException extends DeserializationException {
    @UsedByNative
    public CastIdWrongLengthException() {
      super("Cast ID payload doesn't occupy exactly 32 bytes");
    }
  }

  /** A 32-byte Cast ID. */
  @UsedByNative
  public static final class CastId extends V1DataElement {
    @UsedByNative private final byte[] castId;

    /**
     * Constructs a new Cast ID DE.
     *
     * @throws IllegalArgumentException if the raw bytes of the ID were a length other than 32.
     */
    public CastId(NpAdv npAdv, byte[] castId) {
      this(castId);
      npAdv.ensureLoaded();
    }

    @UsedByNative
    private CastId(byte[] castId) {
      this.castId = castId;
      if (castId.length != 32) {
        throw new IllegalArgumentException("Cast Id is not of length 32");
      }
    }

    @UsedByNative
    public byte[] getCastId() {
      return Arrays.copyOf(castId, castId.length);
    }

    @Override
    public void visit(Visitor v) {
      v.visitCastId(this);
    }

    @Override
    public Generic trySerialize() {
      return nativeTrySerialize(this);
    }

    private native Generic nativeTrySerialize(CastId castId);

    @Override
    public boolean equals(@Nullable Object o) {
      if (o instanceof CastId) {
        CastId other = (CastId) o;
        return Arrays.equals(this.castId, other.castId);
      }
      return false;
    }

    @Override
    public int hashCode() {
      return Arrays.hashCode(this.castId);
    }

    @Override
    public String toString() {
      return "CastIdDE(" + formatByteArrayToString(this.castId, ByteValuePrintFormat.HEX) + ")";
    }
  }

  /**
   * Blanket exception raised when the payload of an Actions DE was unable to be interpreted as a
   * set of actions upon deserialization.
   */
  @UsedByNative
  public static final class ActionsMalformedException extends DeserializationException {
    @UsedByNative
    public ActionsMalformedException() {
      super("Actions DE was malformed");
    }
  }

  /** A collection of one or more "actions" supported by the broadcaster. */
  @UsedByNative
  public static final class Actions extends V1DataElement {
    private final BitSet actionIds;

    /** Constructs an (initially-empty) set of V1 actions. */
    public Actions(NpAdv npAdv) {
      this.actionIds = new BitSet();
      npAdv.ensureLoaded();
    }

    @UsedByNative
    private Actions(byte[] rawBitSet) {
      this.actionIds = BitSet.valueOf(rawBitSet);
    }

    /** Gets a copy of the underlying bit-set of actions. */
    public BitSet getBitSet() {
      return (BitSet) this.actionIds.clone();
    }

    /**
     * Gets a length-256 byte-array which represents a bit set stored in big-endian byte order and
     * little-endian bit order within each byte.
     */
    @UsedByNative
    private byte[] getRawBitSet() {
      return Arrays.copyOf(this.actionIds.toByteArray(), 256);
    }

    /**
     * Determines whether or not the action with the given ID is set within this set of actions.
     *
     * @throws IndexOutOfBoundsException If the action ID is not in the range [0, 2047].
     */
    public boolean hasActionWithId(int actionId) {
      if (actionId < 0 || actionId > 2047) {
        throw new IndexOutOfBoundsException("V1 Action with ID " + actionId + " is out of bounds.");
      }
      return this.actionIds.get(actionId);
    }

    /**
     * Adds the action with the given ID to this set of actions if it was not already present.
     *
     * @throws IndexOutOfBoundsException If the action ID is not in the range [0, 2047].
     */
    public void addActionWithId(int actionId) {
      if (actionId < 0 || actionId > 2047) {
        throw new IndexOutOfBoundsException("V1 Action with ID " + actionId + " is out of bounds.");
      }
      this.actionIds.set(actionId);
    }

    @Override
    public void visit(Visitor v) {
      v.visitActions(this);
    }

    @Override
    public Generic trySerialize() {
      return nativeTrySerialize(this);
    }

    private static native Generic nativeTrySerialize(Actions actions);

    @Override
    public boolean equals(@Nullable Object o) {
      if (o instanceof Actions) {
        Actions other = (Actions) o;
        return this.actionIds.equals(other.actionIds);
      }
      return false;
    }

    @Override
    public int hashCode() {
      return this.actionIds.hashCode();
    }

    @Override
    public String toString() {
      return "V1Actions(" + this.actionIds + ")";
    }
  }

  /**
   * Exception raised upon attempting to deserialize a context sync sequence number whose payload is
   * not exactly one byte.
   */
  @UsedByNative
  public static class ContextSyncSeqNumWrongLengthException extends DeserializationException {
    @UsedByNative
    public ContextSyncSeqNumWrongLengthException() {
      super("Context sync sequence number didn't have a one-byte payload");
    }
  }

  /**
   * Exception raised upon attempting to deserialize a context sync sequence number whose value is
   * not within the supported 0-15 range.
   */
  @UsedByNative
  public static class ContextSyncSeqNumOutOfBoundsException extends DeserializationException {
    @UsedByNative
    public ContextSyncSeqNumOutOfBoundsException() {
      super("Context sync sequence number was not in 0-15");
    }
  }

  /** A context sync sequence number between 0 and 15, inclusive. */
  @UsedByNative
  public static class ContextSyncSeqNum extends V1DataElement {
    @UsedByNative private final int seqNum;

    /**
     * Constructs a new context sync sequence number DE.
     *
     * @throws IllegalArgumentException if the context sync sequence number was not in the range
     *     0-15.
     */
    public ContextSyncSeqNum(NpAdv npAdv, int seqNum) {
      this(seqNum);
      npAdv.ensureLoaded();
    }

    @UsedByNative
    private ContextSyncSeqNum(int seqNum) {
      this.seqNum = seqNum;
      if (seqNum < 0 || seqNum > 15) {
        throw new IllegalArgumentException("Context sync sequence number is out of bounds");
      }
    }

    @UsedByNative
    public int getSeqNum() {
      return seqNum;
    }

    @Override
    public void visit(Visitor v) {
      v.visitContextSyncSeqNum(this);
    }

    @Override
    public Generic trySerialize() {
      return nativeTrySerialize(this);
    }

    private static native Generic nativeTrySerialize(ContextSyncSeqNum seqNum);

    @Override
    public boolean equals(@Nullable Object o) {
      if (o instanceof ContextSyncSeqNum) {
        ContextSyncSeqNum other = (ContextSyncSeqNum) o;
        return this.seqNum == other.seqNum;
      }
      return false;
    }

    @Override
    public int hashCode() {
      return this.seqNum;
    }

    @Override
    public String toString() {
      return "ContextSyncSeqNum(" + this.seqNum + ")";
    }
  }

  /**
   * Exception raised upon attempting to deserialize a deduplication hint whose payload is not
   * exactly 8 bytes.
   */
  @UsedByNative
  public static class DedupHintWrongLengthException extends DeserializationException {
    @UsedByNative
    public DedupHintWrongLengthException() {
      super("Dedup hint payload was not exactly 8 bytes");
    }
  }

  /**
   * A deduplication hint used for determining whether/not an advertisement received across multiple
   * mediums came from the same source.
   */
  @UsedByNative
  public static class DedupHint extends V1DataElement {
    @UsedByNative private final byte[] dedupHint;

    /**
     * Constructs a new dedup hint DE.
     *
     * @throws IllegalArgumentException if the byte payload was not exactly 8 bytes.
     */
    public DedupHint(NpAdv npAdv, byte[] dedupHint) {
      this(dedupHint);
      npAdv.ensureLoaded();
    }

    @UsedByNative
    private DedupHint(byte[] dedupHint) {
      this.dedupHint = dedupHint;
      if (dedupHint.length != 8) {
        throw new IllegalArgumentException("Dedup hint payload was not exactly 8 bytes.");
      }
    }

    @UsedByNative
    public byte[] getBytes() {
      return Arrays.copyOf(dedupHint, dedupHint.length);
    }

    @Override
    public void visit(Visitor v) {
      v.visitDedupHint(this);
    }

    @Override
    public Generic trySerialize() {
      return nativeTrySerialize(this);
    }

    private static native Generic nativeTrySerialize(DedupHint dedupHint);

    @Override
    public boolean equals(@Nullable Object o) {
      if (o instanceof DedupHint) {
        DedupHint other = (DedupHint) o;
        return Arrays.equals(this.dedupHint, other.dedupHint);
      }
      return false;
    }

    @Override
    public int hashCode() {
      return Arrays.hashCode(this.dedupHint);
    }

    @Override
    public String toString() {
      return "DedupHint(" + formatByteArrayToString(this.dedupHint, ByteValuePrintFormat.HEX) + ")";
    }
  }

  /**
   * Exception raised upon attempting to deserialize a device type whose payload is not exactly 1
   * byte.
   */
  @UsedByNative
  public static class DeviceTypeWrongLengthException extends DeserializationException {
    @UsedByNative
    public DeviceTypeWrongLengthException() {
      super("Device type payload was not exactly 1 byte");
    }
  }

  /** The type of device performing the broadcast. */
  public static class DeviceType extends V1DataElement {
    @IntDef({
      Value.UNKNOWN,
      Value.PHONE,
      Value.TABLET,
      Value.DISPLAY,
      Value.LAPTOP,
      Value.TV,
      Value.WATCH,
      Value.CHROMEOS,
      Value.FOLDABLE,
      Value.AUTOMOTIVE,
      Value.SPEAKER,
    })
    @Retention(SOURCE)
    public @interface Value {
      public static final int UNKNOWN = 0;
      public static final int PHONE = 1;
      public static final int TABLET = 2;
      public static final int DISPLAY = 3;
      public static final int LAPTOP = 4;
      public static final int TV = 5;
      public static final int WATCH = 6;
      public static final int CHROMEOS = 7;
      public static final int FOLDABLE = 8;
      public static final int AUTOMOTIVE = 9;
      public static final int SPEAKER = 10;
    }

    @UsedByNative private final int deviceType;

    /**
     * Constructs a new device type DE.
     *
     * @throws IllegalArgumentException if the device type was outside of the recognized range.
     */
    public DeviceType(NpAdv npAdv, int deviceType) {
      this(deviceType);
      npAdv.ensureLoaded();
    }

    @UsedByNative
    private DeviceType(int deviceType) {
      this.deviceType = deviceType;
      if (deviceType < 0 || deviceType > 255) {
        throw new IllegalArgumentException("Device type is out of the allowable range.");
      }
    }

    @UsedByNative
    public int getValue() {
      return deviceType;
    }

    @Override
    public void visit(Visitor v) {
      v.visitDeviceType(this);
    }

    @Override
    public Generic trySerialize() {
      return nativeTrySerialize(this);
    }

    private static native Generic nativeTrySerialize(DeviceType deviceType);

    @Override
    public boolean equals(@Nullable Object o) {
      if (o instanceof DeviceType) {
        DeviceType other = (DeviceType) o;
        return this.deviceType == other.deviceType;
      }
      return false;
    }

    @Override
    public int hashCode() {
      return this.deviceType;
    }

    @Override
    public String toString() {
      String name = "";
      switch (this.deviceType) {
        case 0:
          name = "UNKNOWN";
          break;
        case 1:
          name = "PHONE";
          break;
        case 2:
          name = "TABLET";
          break;
        case 3:
          name = "DISPLAY";
          break;
        case 4:
          name = "LAPTOP";
          break;
        case 5:
          name = "TV";
          break;
        case 6:
          name = "WATCH";
          break;
        case 7:
          name = "CHROMEOS";
          break;
        case 8:
          name = "FOLDABLE";
          break;
        case 9:
          name = "AUTOMOTIVE";
          break;
        case 10:
          name = "SPEAKER";
          break;
        default:
          return "DeviceType(" + this.deviceType + ")";
      }
      return "DeviceType(" + name + ")";
    }
  }

  /**
   * Exception raised upon attempting to deserialize connectivity info whose component ordering,
   * uniqueness, or non-emptiness is violated.
   */
  @UsedByNative
  public static class ConnectivityInfoMalformedException extends DeserializationException {
    @UsedByNative
    public ConnectivityInfoMalformedException() {
      super("Connectivity info had a malformed payload.");
    }
  }

  /**
   * Exception raised upon attempting to deserialize connectivity info with a malformed component.
   */
  @UsedByNative
  public static class ConnectivityInfoComponentMalformedException extends DeserializationException {
    @UsedByNative
    public ConnectivityInfoComponentMalformedException(int index) {
      super("Connectivity info had a malformed payload component at index: " + index);
    }
  }

  /** Information for connecting to a broadcasting device. */
  @UsedByNative
  public static class ConnectivityInfo extends V1DataElement {
    @Nullable private BleConnectivityInfo bleInfo = null;
    @Nullable private WifiLanConnectivityInfo wifiLanInfo = null;

    @UsedByNative
    private ConnectivityInfo(BleConnectivityInfo bleInfo, WifiLanConnectivityInfo wifiLanInfo) {
      this.bleInfo = bleInfo;
      this.wifiLanInfo = wifiLanInfo;
    }

    public ConnectivityInfo(NpAdv npAdv) {
      npAdv.ensureLoaded();
    }

    @Override
    public void visit(Visitor v) {
      v.visitConnectivityInfo(this);
    }

    /** Sets BLE connectivity info, with null indicating that it isn't present. */
    public ConnectivityInfo setBleInfo(@Nullable BleConnectivityInfo bleInfo) {
      this.bleInfo = bleInfo;
      return this;
    }

    /** Sets Wifi-LAN connectivity info, with null indicating that it isn't present. */
    public ConnectivityInfo setWifiLanInfo(@Nullable WifiLanConnectivityInfo wifiLanInfo) {
      this.wifiLanInfo = wifiLanInfo;
      return this;
    }

    /** Gets BLE connectivity info, with null indicating that it isn't present. */
    @UsedByNative
    @Nullable
    public BleConnectivityInfo getBleInfo() {
      return this.bleInfo;
    }

    /** Gets Wifi-LAN connectivity info, with null indicating that it isn't present. */
    @UsedByNative
    @Nullable
    public WifiLanConnectivityInfo getWifiLanInfo() {
      return this.wifiLanInfo;
    }

    @Override
    public Generic trySerialize() {
      return nativeTrySerialize(this);
    }

    private static native Generic nativeTrySerialize(ConnectivityInfo connectivityInfo);

    @Override
    public boolean equals(@Nullable Object o) {
      if (o instanceof ConnectivityInfo) {
        ConnectivityInfo other = (ConnectivityInfo) o;
        return Objects.equals(this.bleInfo, other.bleInfo)
            && Objects.equals(this.wifiLanInfo, other.wifiLanInfo);
      }
      return false;
    }

    @Override
    public int hashCode() {
      return Objects.hash(this.bleInfo, this.wifiLanInfo);
    }

    @Override
    public String toString() {
      String result = "ConnectivityInfo(";
      if (this.bleInfo != null) {
        result += this.bleInfo.toString();
      }
      if (this.bleInfo != null && this.wifiLanInfo != null) {
        result += ", ";
      }
      if (this.wifiLanInfo != null) {
        result += this.wifiLanInfo.toString();
      }
      return result;
    }
  }

  /** Bluetooth Low Energy (BLE) component of the connectivity info DE. */
  @UsedByNative
  public static class BleConnectivityInfo {
    /** Bluetooth MAC address */
    @Nullable private byte[] macAddress = null;

    /**
     * An identifier for the GATT service to connect to.
     *
     * <p>This could be a UUID + instance ID (17 bytes), an attribute handle + instance ID (3
     * bytes), or just a UUID (16 bytes) or attribute handle (2 bytes).
     */
    @Nullable private byte[] gattServiceIdentifier = null;

    /** Two-byte PSM (L2CAP) port to connect on */
    @Nullable private byte[] psm = null;

    /** Two-byte physical identifier ("device token") for the device. */
    @Nullable private byte[] deviceToken = null;

    @UsedByNative
    private BleConnectivityInfo(
        @Nullable byte[] macAddress,
        @Nullable byte[] gattServiceIdentifier,
        @Nullable byte[] psm,
        @Nullable byte[] deviceToken) {
      this.macAddress = macAddress;
      this.gattServiceIdentifier = gattServiceIdentifier;
      this.psm = psm;
      this.deviceToken = deviceToken;
    }

    public BleConnectivityInfo() {}

    /**
     * Sets the 6-byte BLE MAC address.
     *
     * @throws IllegalArgumentException If the passed byte-array was non-null, but of the wrong
     *     length.
     */
    public BleConnectivityInfo setMacAddress(@Nullable byte[] macAddress) {
      if (macAddress != null && macAddress.length != 6) {
        throw new IllegalArgumentException("BLE MAC address should be 6 bytes");
      }
      this.macAddress = macAddress;
      return this;
    }

    /**
     * Sets the 17-bytes-or-less GATT service identifier.
     *
     * @throws IllegalArgumentException If the passed byte-array was non-null, but of the wrong
     *     length.
     */
    public BleConnectivityInfo setGattServiceIdentifier(@Nullable byte[] gattServiceIdentifier) {
      if (gattServiceIdentifier != null && gattServiceIdentifier.length > 17) {
        throw new IllegalArgumentException("GATT service identifier should be under 17 bytes");
      }
      this.gattServiceIdentifier = gattServiceIdentifier;
      return this;
    }

    /**
     * Sets the 2-byte PSM.
     *
     * @throws IllegalArgumentException If the passed byte-array was non-null, but of the wrong
     *     length.
     */
    public BleConnectivityInfo setPsm(@Nullable byte[] psm) {
      if (psm != null && psm.length != 2) {
        throw new IllegalArgumentException("PSM should be 2 bytes");
      }
      this.psm = psm;
      return this;
    }

    /**
     * Sets the 2-byte device token.
     *
     * @throws IllegalArgumentException If the passed byte-array was non-null, but of the wrong
     *     length.
     */
    public BleConnectivityInfo setDeviceToken(@Nullable byte[] deviceToken) {
      if (deviceToken != null && deviceToken.length != 2) {
        throw new IllegalArgumentException("Device Token should be 2 bytes");
      }
      this.deviceToken = deviceToken;
      return this;
    }

    /** Gets the 6-byte BLE Mac address */
    @UsedByNative
    @Nullable
    public byte[] getMacAddress() {
      return this.macAddress;
    }

    /** Gets the sub-17-byte GATT service identifier. */
    @UsedByNative
    @Nullable
    public byte[] getGattServiceIdentifier() {
      return this.gattServiceIdentifier;
    }

    /** Gets the 2-byte PSM. */
    @UsedByNative
    @Nullable
    public byte[] getPsm() {
      return this.psm;
    }

    /** Gets the 2-byte device token. */
    @UsedByNative
    @Nullable
    public byte[] getDeviceToken() {
      return this.deviceToken;
    }

    @Override
    public boolean equals(@Nullable Object o) {
      if (o instanceof BleConnectivityInfo) {
        BleConnectivityInfo other = (BleConnectivityInfo) o;
        return Arrays.equals(this.macAddress, other.macAddress)
            && Arrays.equals(this.gattServiceIdentifier, other.gattServiceIdentifier)
            && Arrays.equals(this.psm, other.psm)
            && Arrays.equals(this.deviceToken, other.deviceToken);
      }
      return false;
    }

    @Override
    public int hashCode() {
      return Arrays.deepHashCode(
          new Object[] {this.macAddress, this.gattServiceIdentifier, this.psm, this.deviceToken});
    }

    @Override
    public String toString() {
      return "BleConnectivityInfo(MAC="
          + formatByteArrayToString(this.macAddress, ByteValuePrintFormat.HEX)
          + ", GattServiceIdentifier="
          + formatByteArrayToString(this.gattServiceIdentifier, ByteValuePrintFormat.HEX)
          + ", PSM="
          + formatByteArrayToString(this.psm, ByteValuePrintFormat.HEX)
          + ", deviceToken="
          + formatByteArrayToString(this.deviceToken, ByteValuePrintFormat.HEX)
          + ")";
    }
  }

  /** Wifi-LAN component of the connectivity info DE. */
  @UsedByNative
  public static class WifiLanConnectivityInfo {
    /** IPv4 (4 bytes) or IPv6 (16 bytes) address. */
    @Nullable private byte[] ipAddress = null;

    /** Two-byte big-endian port number */
    @Nullable private byte[] port = null;

    /** 6-byte BSSID */
    @Nullable private byte[] bssid = null;

    @UsedByNative
    private WifiLanConnectivityInfo(
        @Nullable byte[] ipAddress, @Nullable byte[] port, @Nullable byte[] bssid) {
      this.ipAddress = ipAddress;
      this.port = port;
      this.bssid = bssid;
    }

    public WifiLanConnectivityInfo() {}

    /**
     * Sets the 4 (IPv4) or 16-byte (IPv6) IP address.
     *
     * @throws IllegalArgumentException If the passed byte-array was non-null, but of the wrong
     *     length.
     */
    public WifiLanConnectivityInfo setIpAddress(@Nullable byte[] ipAddress) {
      if (ipAddress != null && !(ipAddress.length == 4 || ipAddress.length == 16)) {
        throw new IllegalArgumentException("IP address should be 4 or 16 bytes in length");
      }
      this.ipAddress = ipAddress;
      return this;
    }

    /**
     * Sets the 2-byte big-endian port number.
     *
     * @throws IllegalArgumentException If the passed byte-array was non-null, but of the wrong
     *     length.
     */
    public WifiLanConnectivityInfo setPort(@Nullable byte[] port) {
      if (port != null && port.length != 2) {
        throw new IllegalArgumentException("Port should be 2 bytes in length");
      }
      this.port = port;
      return this;
    }

    /**
     * Sets the 6-byte BSSID.
     *
     * @throws IllegalArgumentException If the passed byte-array was non-null, but of the wrong
     *     length.
     */
    public WifiLanConnectivityInfo setBssid(@Nullable byte[] bssid) {
      if (bssid != null && bssid.length != 6) {
        throw new IllegalArgumentException("BSSID should be 6 bytes in length");
      }
      this.bssid = bssid;
      return this;
    }

    /** Gets the 4-byte (IPV4) or 16-byte (IPV4) IP address. */
    @UsedByNative
    @Nullable
    public byte[] getIpAddress() {
      return this.ipAddress;
    }

    /** Gets the 2-byte big endian port number */
    @UsedByNative
    @Nullable
    public byte[] getPort() {
      return this.port;
    }

    /** Gets the 6-byte BSSID. */
    @UsedByNative
    @Nullable
    public byte[] getBssid() {
      return this.bssid;
    }

    @Override
    public boolean equals(@Nullable Object o) {
      if (o instanceof WifiLanConnectivityInfo) {
        WifiLanConnectivityInfo other = (WifiLanConnectivityInfo) o;
        return Arrays.equals(this.ipAddress, other.ipAddress)
            && Arrays.equals(this.port, other.port)
            && Arrays.equals(this.bssid, other.bssid);
      }
      return false;
    }

    @Override
    public int hashCode() {
      return Arrays.deepHashCode(new Object[] {this.ipAddress, this.port, this.bssid});
    }

    @Override
    public String toString() {
      return "WifiLanConnectivityInfo(IP="
          + formatByteArrayToString(this.ipAddress, ByteValuePrintFormat.DECIMAL)
          + ", Port="
          + formatByteArrayToString(this.port, ByteValuePrintFormat.HEX)
          + ", BSSID="
          + formatByteArrayToString(this.bssid, ByteValuePrintFormat.HEX)
          + ")";
    }
  }

  private enum ByteValuePrintFormat {
    HEX,
    DECIMAL,
  }

  /** Formats an array of bytes as a list of hexadecimal octet values */
  private static String formatByteArrayToString(
      @Nullable byte[] array, ByteValuePrintFormat format) {
    if (array == null) {
      return "null";
    }
    if (array.length == 0) {
      return "[]";
    }
    boolean first = true;
    // Reserve enough space for up to 5 characters per element (up to 3 digits
    // or up to two hex characters followed by a comma and a space)
    // with the two '[]' brackets at the ends.
    StringBuilder result = new StringBuilder(array.length * 5 + 2);
    for (byte val : array) {
      if (first) {
        result.append("[");
        first = false;
      } else {
        result.append(", ");
      }
      switch (format) {
        case HEX:
          result.append(String.format("%02x", Byte.toUnsignedInt(val)));
          break;
        case DECIMAL:
          result.append("" + Byte.toUnsignedInt(val));
          break;
        default:
          break;
      }
    }
    result.append("]");
    return result.toString();
  }
}
