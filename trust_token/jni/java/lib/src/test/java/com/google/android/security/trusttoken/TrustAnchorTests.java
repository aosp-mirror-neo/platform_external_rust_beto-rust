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

import static org.junit.Assert.assertThrows;

import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

import java.util.Base64;
import java.util.List;

@RunWith(JUnit4.class)
public final class TrustAnchorTests {
    static final byte[] ROOT_KEY =
            Base64.getDecoder()
                    .decode("owECIAYhWCBvxFYpAr0HqX26+VcNbYzByA+hgDGZGkN5Z4lTvVE0eQ==".getBytes());
    static final byte[] INTERMEDIATE_CERTIFICATE =
            Base64.getDecoder()
                    .decode(
                            "hEOhASehBE9Bc3ltbWV0cmljRWREU0FYVKYBZlJvb3RDYQJsQVFJREJBVUdCd2dKBBr/////BgE6AKu+tFgoowECIAYhWCApwprQfS8rTVIgP0bX97hALape+sCUd12KoMp6ExLdcDoAq762AVhAx3u65K5sIDFKg77sXAZcBPLFpnhkhZCsGw/z8gEVXtnw8WWssJrifrs5lxE0W3Qide02ys6PYxAFfvyQTsUSBw=="
                                    .getBytes());
    static final byte[] TRUST_TOKEN =
            Base64.getDecoder()
                    .decode(
                            "hEOhASehBE9Bc3ltbWV0cmljRWREU0FYa6YBbEFRSURCQVVHQndnSgQa/////wYBOgCrvrRYICoqKioqKioqKioqKioqKioqKioqKioqKioqKioqKioqOgCrvrVYIGNjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjOgCrvrMCWEAWF/YwfLiNY3G3O+1Ex/K5hQOSSytBggxGhIx43JT+n/iP5sE7G/rWLEKBFNrGMXFxUMv2NZ8QG6P+W18pGasH"
                                    .getBytes());

    @Test
    public void testTrustAnchor_new() {
        // No exception is pass.
        try (var trustAnchor =
                new TrustAnchor(List.of(ROOT_KEY), List.of(INTERMEDIATE_CERTIFICATE))) {}
    }

    @Test
    public void testTrustAnchor_close() {
        // No exception is pass.
        var trustAnchor = new TrustAnchor(List.of(ROOT_KEY), List.of(INTERMEDIATE_CERTIFICATE));
        // close() should be idempotent.
        trustAnchor.close();
        trustAnchor.close();
        assertThrows(IllegalStateException.class, () -> new TrustToken(trustAnchor, TRUST_TOKEN));
    }

    @Test
    public void testTrustAnchor_invalid_root_key() {
        assertThrows(
                IllegalArgumentException.class,
                () ->
                        new TrustAnchor(
                                List.of("invalid-root-key".getBytes()),
                                List.of(INTERMEDIATE_CERTIFICATE)));
    }

    @Test
    public void testTrustAnchor_invalid_intermediate_certificate() {
        assertThrows(
                IllegalArgumentException.class,
                () ->
                        new TrustAnchor(
                                List.of(ROOT_KEY),
                                List.of("invalid-intermediate-cert".getBytes())));
    }
}
