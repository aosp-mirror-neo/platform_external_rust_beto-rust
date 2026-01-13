/*
 * Copyright 2025 Google LLC
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

package com.google.android.security.trusttoken;

import java.time.Instant;

public final class TrustToken implements AutoCloseable {
    private final long mHandle;
    private boolean mClosed = false;

    static {
        System.loadLibrary("trust_token_jni");
    }

    public TrustToken(TrustAnchor trustAnchor, byte[] encoded) {
        mHandle = native_init(trustAnchor.getHandle(), encoded);
    }

    @Override
    public void close() {
        if (!mClosed) {
            native_release(mHandle);
            mClosed = true;
        }
    }

    public Instant getExpirationTime() {
        return Instant.ofEpochMilli(native_get_expiration_time_millis(mHandle));
    }

    public Instant getIssuedAt() {
        return Instant.ofEpochMilli(native_get_issued_at_millis(mHandle));
    }

    public byte[] getPublicKey() {
        return native_get_public_key(mHandle);
    }

    public TrustTokenPolicy getPolicy() {
        return native_get_policy(mHandle);
    }

    public boolean verifyChallenge(byte[] challenge, byte[] remoteResponse) {
        return native_verify_challenge(mHandle, challenge, remoteResponse);
    }

    private static native long native_init(long trustAnchorHandle, byte[] encoded);

    private static native void native_release(long handle);

    private static native long native_get_expiration_time_millis(long handle);

    private static native long native_get_issued_at_millis(long handle);

    private static native byte[] native_get_public_key(long handle);

    private static native TrustTokenPolicy native_get_policy(long handle);

    private static native boolean native_verify_challenge(
            long handle, byte[] challenge, byte[] remoteResponse);
}
