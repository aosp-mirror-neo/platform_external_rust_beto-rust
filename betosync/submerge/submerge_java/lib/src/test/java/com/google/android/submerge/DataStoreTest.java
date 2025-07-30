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

import static com.google.common.truth.Truth.assertThat;
import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.Assert.assertThrows;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.doThrow;
import static org.mockito.Mockito.only;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.verifyNoInteractions;
import static org.mockito.Mockito.when;

import androidx.annotation.Nullable;
import com.google.android.submerge.StorageInterface.StorageException;
import com.google.android.submerge.SubmergeDataType.SubmergeDataTypeHandleId;
import com.google.errorprone.annotations.MustBeClosed;
import java.util.HashMap;
import org.junit.After;
import org.junit.Before;
import org.junit.Rule;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;
import org.mockito.ArgumentCaptor;
import org.mockito.Mock;
import org.mockito.Mockito;
import org.mockito.Spy;
import org.mockito.junit.MockitoJUnit;
import org.mockito.junit.MockitoRule;

@RunWith(JUnit4.class)
public class DataStoreTest {
  @Rule public MockitoRule mockitoRule = MockitoJUnit.rule();

  @Mock private NetworkInterface networkInterface;

  @Spy StorageInterface storageInterface = new Storage();

  @Mock private TimestampProvider timestampProvider;

  private DataStore<String> dataStore;

  @Before
  @SuppressWarnings("MustBeClosed")
  public void setUp() throws StorageException {
    Mockito.lenient()
        .when(timestampProvider.now())
        .thenAnswer((unused) -> System.currentTimeMillis() / 1000L);
    this.dataStore = createDataStore("test_actor");
  }

  @MustBeClosed
  private DataStore<String> createDataStore(String actor) {
    return new DataStore<>(
        actor, networkInterface, storageInterface, timestampProvider, new StringConverter());
  }

  @After
  public void cleanUp() {
    this.dataStore.close();
  }

  @Test
  public void testOpenDocument() throws TransactionException {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      assertThat(doc.getRoot()).isNull();
      assertThat(doc.getDocumentId()).isEqualTo("mydoc");
      doc.commit();
    }
  }

  @Test
  public void testCommitTwice() throws TransactionException {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      doc.commit();
      RuntimeException e = assertThrows(RuntimeException.class, doc::commit);
      assertThat(e).hasMessageThat().isEqualTo("Cannot commit the same transaction twice");
    }
  }

  @SuppressWarnings("MustBeClosed")
  @Test
  public void newDataType_whileClosed_shouldThrow() throws Exception {
    DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc");
    doc.close();

    assertThrows(IllegalStateException.class, () -> doc.newMap());
    assertThrows(IllegalStateException.class, () -> doc.newSet());
    assertThrows(IllegalStateException.class, () -> doc.newRegister());
    assertThrows(IllegalStateException.class, () -> doc.newVectorData());
  }

  @Test
  public void testToString() {
    assertThat(dataStore.toString()).isNotEmpty();
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      assertThat(doc.toString()).isNotEmpty();
      try (SubmergeMap<String> map = doc.newMap()) {
        assertThat(map.toString()).isNotEmpty();
      }
      try (SubmergeRegister<String> register = doc.newRegister()) {
        assertThat(register.toString()).isNotEmpty();
      }
      try (SubmergeSet<String> set = doc.newSet()) {
        assertThat(set.toString()).isNotEmpty();
      }
      try (SubmergeVectorData<String> vectorData = doc.newVectorData()) {
        assertThat(vectorData.toString()).isNotEmpty();
      }
    }
  }

  @Test
  public void testClose() {
    // Close should be idempotent
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      try (SubmergeMap<String> map = doc.newMap()) {
        map.close();
        map.close();
      }
      try (SubmergeRegister<String> register = doc.newRegister()) {
        register.close();
        register.close();
      }
      try (SubmergeSet<String> set = doc.newSet()) {
        set.close();
        set.close();
      }
      try (SubmergeVectorData<String> vectorData = doc.newVectorData()) {
        vectorData.close();
        vectorData.close();
      }
      dataStore.close();
      dataStore.close();
      doc.close();
      doc.close();
    }
    verifyNoInteractions(networkInterface);
  }

  @Test
  public void testGetRoot() throws TransactionException {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      assertThat(doc.getRoot()).isNull();
      doc.commit();
    }
  }

  @Test
  public void testSetRoot() throws TimestampOverflowException, TransactionException {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc");
        SubmergeMap<String> map = doc.newMap()) {
      doc.setRoot(map);
      assertThat(doc.getRoot()).isInstanceOf(SubmergeMap.class);
      doc.commit();
    }
  }

  @Test
  public void testSetRootNull() throws TimestampOverflowException, TransactionException {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc");
        SubmergeMap<String> map = doc.newMap()) {
      doc.setRoot(map);
      assertThat(doc.getRoot()).isInstanceOf(SubmergeMap.class);
      doc.setRoot(null);
      assertThat(doc.getRoot()).isNull();
      doc.commit();
    }
  }

  @Test
  public void testSetRootTimestampOverflow()
      throws TimestampOverflowException, TransactionException {
    // Always return the same wall-clock timestamp to trigger timestamp overflow
    when(timestampProvider.now()).thenReturn(1L);
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      for (int i = 0; i < 256; i++) {
        doc.setRoot(doc.newMap());
      }
      assertThrows(TimestampOverflowException.class, () -> doc.setRoot(doc.newMap()));
    }
  }

  @Test
  public void testSet()
      throws TimestampOverflowException, GenerationExhaustedException, TransactionException {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      SubmergeSet<String> set = doc.newSet();
      set.add("Hello");
      set.add("World");
      doc.setRoot(set);

      SubmergeSet<String> actualSet = (SubmergeSet<String>) doc.getRoot();
      assertThat(actualSet.contains("Hello")).isTrue();
      assertThat(actualSet.contains("World")).isTrue();
      assertThat(actualSet.contains("Mars")).isFalse();

      assertThat(actualSet.entries()).containsExactly("Hello", "World");

      actualSet.remove("World");
      assertThat(actualSet.contains("World")).isFalse();
      doc.commit();
      verify(networkInterface, only()).onNewUpdate(anyString(), any());
    }
  }

  @Test
  public void testSetGenerationExhausted() throws Exception {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      SubmergeSet<String> set = doc.newSet();
      set.bumpGenerationNumberForTesting("Hello", (int) 4294967295L);
      set.remove("Hello");
      assertThrows(GenerationExhaustedException.class, () -> set.add("Hello"));
    }
  }

  @Test
  public void testRegister() throws Exception {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      SubmergeRegister<String> reg = doc.newRegister();
      reg.set("Hello world");
      doc.setRoot(reg);
      assertThat(doc.getRoot()).isInstanceOf(SubmergeRegister.class);
      String registerValue = ((SubmergeRegister<String>) doc.getRoot()).get();
      assertThat(registerValue).isEqualTo("Hello world");
      doc.commit();
      verify(networkInterface, only()).onNewUpdate(anyString(), any());
    }
    Mockito.reset(networkInterface);

    // Reopen the doc and check that the set value is still there
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      SubmergeRegister<String> reg = (SubmergeRegister<String>) doc.getRoot();
      String registerValue = reg.get();
      assertThat(registerValue).isEqualTo("Hello world");
    }
  }

  @Test
  public void testRegisterTimestampOverflow() throws Exception {
    // Always return the same wall-clock timestamp to trigger timestamp overflow
    when(timestampProvider.now()).thenReturn(1L);
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      SubmergeRegister<String> register = doc.newRegister();
      for (int i = 0; i < 256; i++) {
        register.set(i + "");
      }
      assertThrows(TimestampOverflowException.class, () -> register.set("257"));
    }
  }

  @Test
  public void testVectorData() throws Exception {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      doc.setRoot(doc.newVectorData());
      SubmergeVectorData<String> child = (SubmergeVectorData<String>) doc.getRoot();
      child.set("hello I am testy");
      assertThat(child.entries()).hasSize(1);
      assertThat(child.entries().get("nonexistent")).isNull();
      assertThat(child.entries()).containsEntry("test_actor", "hello I am testy");
      doc.commit();
    }
  }

  @Test
  public void testVectorDataVersionOverflow() throws Exception {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      SubmergeVectorData<String> vectorData = doc.newVectorData();
      vectorData.bumpVersionNumberForTesting((int) 4294967295L);
      assertThrows(VersionOverflowException.class, () -> vectorData.set("new data"));
    }
  }

  @Test
  public void testMap() throws TimestampOverflowException, TransactionException {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      doc.setRoot(doc.newMap());
      SubmergeMap<String> map = (SubmergeMap<String>) doc.getRoot();
      SubmergeRegister<String> child1 = doc.newRegister();
      child1.set("Hello");
      map.setChild("child1", child1);
      SubmergeVectorData<String> child2 = doc.newVectorData();
      map.setChild("child2", child2);

      SubmergeRegister<String> actualChild1 = (SubmergeRegister<String>) map.getChild("child1");
      assertThat(actualChild1.get()).isEqualTo("Hello");

      SubmergeVectorData<String> actualChild2 = (SubmergeVectorData<String>) map.getChild("child2");
      assertThat(actualChild2.entries()).isEmpty();

      doc.commit();
    }
  }

  @Test
  public void testPassNonOwnedChild() throws TimestampOverflowException, TransactionException {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc");
        DataStore<String> dataStore2 = createDataStore("test_actor2");
        DocumentTransaction<String> doc2 = dataStore2.newDocumentTransaction("mydoc2")) {
      SubmergeMap<String> child = doc.newMap();
      doc.setRoot(child);
      assertThrows(OwnershipException.class, () -> doc2.setRoot(child));
      doc.commit();
      doc2.commit();
    }
  }

  @Test
  public void testPassChildFromDifferentDoc()
      throws TimestampOverflowException, TransactionException {
    try (DataStore<String> dataStore2 = createDataStore("test_actor2");
        DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc");
        DocumentTransaction<String> doc2 = dataStore2.newDocumentTransaction("mydoc2")) {
      SubmergeMap<String> child = doc.newMap();
      doc.setRoot(child);
      assertThrows(OwnershipException.class, () -> doc2.setRoot(doc.getRoot()));
      doc.commit();
      doc2.commit();
    }
  }

  @Test
  public void testOnNewUpdateStorageError()
      throws TimestampOverflowException, TransactionException, StorageException {
    doThrow(StorageException.class).when(storageInterface).onNewUpdate(anyString(), any());

    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      doc.setRoot(doc.newMap());
      TransactionException e = assertThrows(TransactionException.class, doc::commit);
      assertThat(e).hasCauseThat().isInstanceOf(StorageException.class);
    }
  }

  @Test
  public void testCommitNetworkUpdate() throws Exception {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      SubmergeRegister<String> child = doc.newRegister();
      child.set("Hello");
      doc.setRoot(child);
      doc.commit();
    }

    ArgumentCaptor<byte[]> captor = ArgumentCaptor.forClass(byte[].class);
    verify(networkInterface).onNewUpdate(eq("mydoc"), captor.capture());

    // Pretend the following runs on another device
    try (DataStore<String> dataStore2 =
        new DataStore<>(
            "test_actor2",
            networkInterface,
            new Storage(),
            timestampProvider,
            new StringConverter())) {
      dataStore2.commitNetworkUpdate("mydoc", captor.getValue());
      try (DocumentTransaction<String> doc2 = dataStore2.newDocumentTransaction("mydoc")) {
        SubmergeRegister<String> register = (SubmergeRegister<String>) doc2.getRoot();
        assertThat(register.get()).isEqualTo("Hello");
      }
    }
  }

  @Test
  public void testCommitFullUpdate() throws Exception {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      SubmergeRegister<String> child = doc.newRegister();
      child.set("Hello");
      doc.setRoot(child);
      doc.commit();
    }

    ArgumentCaptor<byte[]> captor = ArgumentCaptor.forClass(byte[].class);
    verify(networkInterface).onNewUpdate(eq("mydoc"), captor.capture());

    assertThat(dataStore.getFullUpdateMessage("mydoc")).isEqualTo(captor.getValue());

    // Pretend the following runs on another device
    try (DataStore<String> dataStore2 =
        new DataStore<>(
            "test_actor2",
            networkInterface,
            new Storage(),
            timestampProvider,
            new StringConverter())) {
      dataStore2.commitNetworkUpdate("mydoc", dataStore.getFullUpdateMessage("mydoc"));
      try (DocumentTransaction<String> doc2 = dataStore2.newDocumentTransaction("mydoc")) {
        SubmergeRegister<String> register = (SubmergeRegister<String>) doc2.getRoot();
        assertThat(register.get()).isEqualTo("Hello");
      }
    }
  }

  @Test
  public void testOlderBaseNeeded() throws Exception {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      SubmergeRegister<String> child = doc.newRegister();
      child.set("Hello");
      doc.setRoot(child);
      doc.commit();
    }

    Mockito.reset(networkInterface);

    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      SubmergeRegister<String> child = doc.newRegister();
      child.set("Hi there");
      doc.setRoot(child);
      doc.commit();
    }

    ArgumentCaptor<byte[]> captor = ArgumentCaptor.forClass(byte[].class);
    verify(networkInterface).onNewUpdate(eq("mydoc"), captor.capture());

    // Pretend the following runs on another device
    try (DataStore<String> dataStore2 =
        new DataStore<>(
            "test_actor2",
            networkInterface,
            new Storage(),
            timestampProvider,
            new StringConverter())) {
      OlderBaseNeededException e =
          assertThrows(
              OlderBaseNeededException.class,
              () -> dataStore2.commitNetworkUpdate("mydoc", captor.getValue()));

      byte[] deltaMessage = dataStore.calculateDelta("mydoc", e.getBaseVersionNeeded());
      dataStore2.commitNetworkUpdate("mydoc", deltaMessage);

      try (DocumentTransaction<String> doc2 = dataStore2.newDocumentTransaction("mydoc")) {
        SubmergeRegister<String> register = (SubmergeRegister<String>) doc2.getRoot();
        assertThat(register.get()).isEqualTo("Hi there");
      }
    }
  }

  @Test
  public void testReadFromStorage() throws Exception {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      SubmergeRegister<String> child = doc.newRegister();
      child.set("Hello");
      doc.setRoot(child);
      doc.commit();
    }

    ArgumentCaptor<byte[]> captor = ArgumentCaptor.forClass(byte[].class);
    verify(storageInterface).onNewUpdate(eq("mydoc"), captor.capture());

    Storage storage2 = new Storage();
    storage2.map.put("mydoc", captor.getValue());

    try (DataStore<String> dataStore2 =
        new DataStore<>(
            "test_actor2", networkInterface, storage2, timestampProvider, new StringConverter())) {
      try (DocumentTransaction<String> doc2 = dataStore2.newDocumentTransaction("mydoc")) {
        SubmergeRegister<String> register = (SubmergeRegister<String>) doc2.getRoot();
        assertThat(register.get()).isEqualTo("Hello");
      }
    }
  }

  @Test
  public void testMapSetChildBeforeAddingToDocument() throws Exception {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      SubmergeMap<String> map = doc.newMap();
      SubmergeRegister<String> register = doc.newRegister();
      register.set("test");
      assertThrows(OwnershipException.class, () -> map.setChild("key1", register));
    }
  }

  @Test
  public void testGetDocumentVersion() throws Exception {
    String firstMessage =
        "Hello this is the first message, deliberately long to make sure its "
            + "delta message is longer than the second transaction";

    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      SubmergeMap<String> map = doc.newMap();
      SubmergeRegister<String> register = doc.newRegister();
      register.set(firstMessage);
      doc.setRoot(map);
      map.setChild("key1", register);
      doc.commit();
    }

    byte[] updateMessage1, updateMessage2;

    Storage storage2 = new Storage();

    try (DataStore<String> dataStore2 =
        new DataStore<>(
            "test_actor2", networkInterface, storage2, timestampProvider, new StringConverter())) {
      updateMessage1 = dataStore.calculateDelta("mydoc", dataStore2.getDocumentVersion("mydoc"));
      dataStore2.commitNetworkUpdate("mydoc", updateMessage1);
      try (DocumentTransaction<String> doc2 = dataStore2.newDocumentTransaction("mydoc")) {
        SubmergeMap<String> map = (SubmergeMap<String>) doc2.getRoot();
        SubmergeRegister<String> register = (SubmergeRegister<String>) map.getChild("key1");
        assertThat(register.get()).isEqualTo(firstMessage);
      }
    }

    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      SubmergeMap<String> map = (SubmergeMap<String>) doc.getRoot();
      SubmergeRegister<String> register = doc.newRegister();
      register.set("Bye");
      map.setChild("key2", register);
      doc.commit();
    }

    try (DataStore<String> dataStore2 =
        new DataStore<>(
            "test_actor2", networkInterface, storage2, timestampProvider, new StringConverter())) {
      updateMessage2 = dataStore.calculateDelta("mydoc", dataStore2.getDocumentVersion("mydoc"));
      dataStore2.commitNetworkUpdate("mydoc", updateMessage2);
      try (DocumentTransaction<String> doc2 = dataStore2.newDocumentTransaction("mydoc")) {
        SubmergeMap<String> map = (SubmergeMap<String>) doc2.getRoot();
        SubmergeRegister<String> register1 = (SubmergeRegister<String>) map.getChild("key1");
        SubmergeRegister<String> register2 = (SubmergeRegister<String>) map.getChild("key2");
        assertThat(register1.get()).isEqualTo(firstMessage);
        assertThat(register2.get()).isEqualTo("Bye");
      }
    }

    // Since the first update contains a long message, its message length should be longer than the
    // second update if delta calculation is working correctly.
    assertThat(updateMessage1.length).isGreaterThan(updateMessage2.length);
  }

  @Test
  public void testSubmergeDataTypeCreate() {
    try (DocumentTransaction<String> doc = dataStore.newDocumentTransaction("mydoc")) {
      {
        SubmergeRegister<String> register = doc.newRegister();
        @SubmergeDataTypeHandleId long handle = register.takeHandle();
        assertThat(SubmergeDataType.create(handle, new StringConverter()))
            .isInstanceOf(SubmergeRegister.class);
      }

      {
        SubmergeMap<String> map = doc.newMap();
        @SubmergeDataTypeHandleId long handle = map.takeHandle();
        assertThat(SubmergeDataType.create(handle, new StringConverter()))
            .isInstanceOf(SubmergeMap.class);
      }

      {
        SubmergeSet<String> set = doc.newSet();
        @SubmergeDataTypeHandleId long handle = set.takeHandle();
        assertThat(SubmergeDataType.create(handle, new StringConverter()))
            .isInstanceOf(SubmergeSet.class);
      }

      {
        SubmergeVectorData<String> vectorData = doc.newVectorData();
        @SubmergeDataTypeHandleId long handle = vectorData.takeHandle();
        assertThat(SubmergeDataType.create(handle, new StringConverter()))
            .isInstanceOf(SubmergeVectorData.class);
      }
    }
  }

  private static class StringConverter implements Converter<String> {

    @Override
    public String deserialize(byte[] bytes) {
      return new String(bytes, UTF_8);
    }

    @Override
    public byte[] serialize(String value) {
      return value.getBytes(UTF_8);
    }
  }

  private static class Storage implements StorageInterface {

    private final HashMap<String, byte[]> map = new HashMap<>();

    @Override
    public void onNewUpdate(String docId, byte[] serializedDoc) throws StorageException {
      map.put(docId, serializedDoc);
    }

    @Nullable
    @Override
    public byte[] readFromStorage(String docId) throws StorageException {
      return map.get(docId);
    }
  }
}
