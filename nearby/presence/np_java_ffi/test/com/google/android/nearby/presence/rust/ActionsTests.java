/*
 * Copyright 2023 Google LLC
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
public class ActionsTests {
  private static final int NUM_RANDOM_TRIALS = 1000;

  private final NpAdv npAdv = new NpAdv();

  /**
   * A test which repeatedly round-trips a random collection of (up to) 64 actions in the range [0,
   * 255], from bitmap to V1 DE to bitmap.
   */
  @Test
  public void roundtripRandomV1Actions() throws Exception {
    Random random = new Random(12345);
    for (int trial = 0; trial < NUM_RANDOM_TRIALS; trial++) {
      V1DataElement.Actions actions = new V1DataElement.Actions(npAdv);

      // Populate the actions bitmap.
      int numActions = random.nextInt(63) + 1; // Needs to be non-zero.
      for (int i = 0; i < numActions; i++) {
        int actionId = random.nextInt(256);
        actions.addActionWithId(actionId);
      }

      // Serialize to a DE.
      V1DataElement.Generic genericDe = actions.trySerialize();

      // Deserialize from the DE.
      V1DataElement de = genericDe.tryDeserialize();
      V1DataElement.Actions reconstructedActions = (V1DataElement.Actions) de;

      // Check that the two actions-sets match on every action.
      for (int actionId = 0; actionId <= 2047; actionId++) {
        assertThat(actions.hasActionWithId(actionId))
            .isEqualTo(reconstructedActions.hasActionWithId(actionId));
      }
    }
  }

  /**
   * A test which verifies that a TooMuchDataException is thrown for a fully-populated actions
   * bitmap.
   */
  @Test(expected = V1DataElement.TooMuchDataException.class)
  public void actionsTooMuchDataException() throws Exception {
    V1DataElement.Actions actions = new V1DataElement.Actions(npAdv);
    for (int actionId = 0; actionId <= 2047; actionId++) {
      actions.addActionWithId(actionId);
    }
    V1DataElement.Generic unused = actions.trySerialize();
  }
}
