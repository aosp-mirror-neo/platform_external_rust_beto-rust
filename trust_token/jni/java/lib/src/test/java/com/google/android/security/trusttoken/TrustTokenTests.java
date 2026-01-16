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

import static com.google.common.truth.Truth.assertThat;

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import java.io.IOException;
import java.util.List;

@RunWith(JUnit4.class)
public final class TrustTokenTests {
    static final byte[] ROOT_KEY = testdata("root_1_public");
    static final byte[] OTHER_ROOT_KEY = testdata("root_2_public");
    static final byte[] INTERMEDIATE_CERTIFICATE = testdata("root_1_intermediate_1");
    static final byte[] OTHER_INTERMEDIATE_CERTIFICATE = testdata("root_2_intermediate_1");
    static final byte[] TRUST_TOKEN = testdata("root_1_intermediate_1_token_1");

    static byte[] testdata(String path) {
        try {
            return Thread.currentThread()
                    .getContextClassLoader()
                    .getResourceAsStream(path)
                    .readAllBytes();
        } catch (IOException e) {
            return null;
        }
    }

    @Test
    public void testTrustToken_attributes() {
        try (var trustAnchor =
                        new TrustAnchor(List.of(ROOT_KEY), List.of(INTERMEDIATE_CERTIFICATE));
                var token = new TrustToken(trustAnchor, TRUST_TOKEN)) {
            assertThat(token.getExpirationTime().toEpochMilli()).isEqualTo(4294967295000L);
            assertThat(token.getIssuedAt().toEpochMilli()).isEqualTo(0);
            assertThat(token.getPublicKey())
                    .isEqualTo(testdata("root_1_intermediate_1_token_1_key_public"));
            assertThat(token.getPolicy()).isEqualTo(TrustTokenPolicy.VERIFIED_DEVICE_STRONG);
        }
    }

    @Test
    public void testTrustToken_close() {
        try (var trustAnchor =
                new TrustAnchor(List.of(ROOT_KEY), List.of(INTERMEDIATE_CERTIFICATE))) {
            var token = new TrustToken(trustAnchor, TRUST_TOKEN);
            // close() should be idempotent.
            token.close();
            token.close();
            assertThrows(IllegalStateException.class, () -> token.getIssuedAt());
        }
    }

    @Test
    public void testTrustToken_invalidEncoding() {
        try (var trustAnchor =
                new TrustAnchor(List.of(ROOT_KEY), List.of(INTERMEDIATE_CERTIFICATE))) {
            assertThrows(
                    IllegalArgumentException.class,
                    () -> new TrustToken(trustAnchor, "some-random-stuff".getBytes()));
        }
    }

    @Test
    public void testTrustToken_invalidSignature() {
        try (var trustAnchor =
                new TrustAnchor(List.of(OTHER_ROOT_KEY), List.of(OTHER_INTERMEDIATE_CERTIFICATE))) {
            assertThrows(
                    TrustTokenInvalidSignatureException.class,
                    () -> new TrustToken(trustAnchor, TRUST_TOKEN));
        }
    }

    @Test
    public void testVerifyChallenge_success() {
        byte[] challenge = testdata("challenge_1");
        byte[] correctResponse = testdata("root_1_intermediate_1_token_1_challenge_1_response");
        try (var trustAnchor =
                        new TrustAnchor(List.of(ROOT_KEY), List.of(INTERMEDIATE_CERTIFICATE));
                var token = new TrustToken(trustAnchor, TRUST_TOKEN)) {
            assertTrue(token.verifyChallenge(challenge, correctResponse));
            assertFalse(token.verifyChallenge(challenge, "incorrect-response".getBytes()));
        }
    }
}
