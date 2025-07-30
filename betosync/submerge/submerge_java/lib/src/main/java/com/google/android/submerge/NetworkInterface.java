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

package com.google.android.submerge;

/**
 * The network interface for a {@link DataStore}.
 *
 * <p>When a {@link DocumentTransaction} is committed, updated messages will be generated and passed
 * to {@link #onNewUpdate(String, byte[])}. The network interface should make a best-effort to send
 * the update messages to other nodes on the network, retrying as desired.
 *
 * <p>It is safe for the network interface to send the same messages multiple times, or out of
 * order. Eventual consistency of the documents are guaranteed as long as the update messages are
 * eventually delivered.
 *
 * <p>The data store will not wait for the network message to be sent or delivered. If the network
 * layer wishes to get an update message on demand, it can call {@link
 * DataStore#calculateDelta(String, VersionVector)} or {@link
 * DataStore#getFullUpdateMessage(String)}.
 */
public interface NetworkInterface {
  /**
   * Notifies the network interface that a new update is available.
   *
   * <p>This is called when a new local modification is committed to the datastore. Implementations
   * for this method must be synchronize and return quickly. If the implementation needs to spend
   * longer time to, say, talk to the server, it should copy the bytes to be processed later and
   * return from this method.
   *
   * @param docId The document ID the update is for.
   * @param updateMessage The serialized update message. This update message can be sent over the
   *     network to a peer device to be committed to the remote database using {@link
   *     DataStore#commitNetworkUpdate(String, byte[])}. This contains enough information to update
   *     a document that is at least as new as the local document. If this is not new enough, an
   *     exception {@link OlderBaseNeededException} will be thrown when {@link
   *     DataStore#commitNetworkUpdate(String, byte[])} is called.
   */
  void onNewUpdate(String docId, byte[] updateMessage);
}
