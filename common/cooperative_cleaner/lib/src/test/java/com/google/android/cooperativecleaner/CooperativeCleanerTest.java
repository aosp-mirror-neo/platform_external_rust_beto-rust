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

package com.google.android.cooperativecleaner;

import static com.google.common.truth.Truth.assertThat;
import static org.mockito.Mockito.verify;

import com.google.android.cooperativecleaner.CooperativeCleaner.Registration;
import java.lang.ref.PhantomReference;
import java.lang.ref.ReferenceQueue;
import org.junit.Rule;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;
import org.mockito.Mock;
import org.mockito.junit.MockitoJUnit;
import org.mockito.junit.MockitoRule;
import org.mockito.quality.Strictness;

@RunWith(JUnit4.class)
public class CooperativeCleanerTest {

  @Rule public MockitoRule mockito = MockitoJUnit.rule().strictness(Strictness.STRICT_STUBS);

  @Mock Runnable cleanupAction;

  @Test
  public void closeRegistration_callsCleanupAction() throws Exception {
    Object object = new Object();
    CooperativeCleaner cooperativeCleaner = new CooperativeCleaner();
    Registration registration = cooperativeCleaner.register(object, cleanupAction);
    registration.close();

    verify(cleanupAction).run();
  }

  @Test
  public void gc_callsCleanupAction() throws Exception {
    CooperativeCleaner cooperativeCleaner = new CooperativeCleaner();
    waitForGc(createCleanerRegisteredObject(cooperativeCleaner));

    cooperativeCleaner.processQueuedObjects();
    verify(cleanupAction).run();
  }

  private Object createCleanerRegisteredObject(CooperativeCleaner cooperativeCleaner) {
    Object object = new Object();
    Registration unused = cooperativeCleaner.register(object, cleanupAction);
    return object;
  }

  private void waitForGc(Object object) throws InterruptedException {
    ReferenceQueue<Object> queue = new ReferenceQueue<>();
    // Need to keep this around to be notified when object is GC'd
    PhantomReference<Object> ref = new PhantomReference<>(object, queue);

    object = null;
    System.gc();

    assertThat(queue.remove(5000)).isSameInstanceAs(ref);
  }
}
