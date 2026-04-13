/*
 * Copyright 2021 Beyond Scale Group
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

package edomata.java;

import java.time.Instant;
import java.util.Arrays;
import java.util.List;

/**
 * Java integration test that verifies the edomata Java API compiles and
 * works correctly from pure Java code.
 *
 * This class is compiled by javac as part of the test suite, proving
 * that all Java-facing types are usable without any Scala knowledge.
 */
public class JavaApiTest {

    // -- JEither tests --

    public static void testJEitherRight() {
        JEither<String, Integer> either = JEither.right(42);
        assert either.isRight() : "Expected Right";
        assert !either.isLeft() : "Expected not Left";
        assert either.getRight().equals(42) : "Expected value 42";
    }

    public static void testJEitherLeft() {
        JEither<String, Integer> either = JEither.left("error");
        assert either.isLeft() : "Expected Left";
        assert !either.isRight() : "Expected not Right";
        assert either.getLeft().equals("error") : "Expected 'error'";
    }

    public static void testJEitherFold() {
        JEither<String, Integer> right = JEither.right(10);
        String result = right.<String, Integer, String>fold(
            l -> "left:" + l,
            r -> "right:" + r
        );
        assert result.equals("right:10") : "Expected 'right:10', got: " + result;
    }

    // -- JDecision tests --

    public static void testJDecisionAccept() {
        JDecision<String, String, ?> dec = JDecision.accept("event1", "event2");
        assert dec.isAccepted() : "Expected Accepted";
        assert !dec.isRejected() : "Expected not Rejected";
        assert !dec.isIndecisive() : "Expected not Indecisive";
    }

    public static void testJDecisionReject() {
        JDecision<String, String, ?> dec = JDecision.reject("reason1");
        assert dec.isRejected() : "Expected Rejected";
    }

    public static void testJDecisionPure() {
        JDecision<String, String, String> dec = JDecision.pure("hello");
        assert dec.isIndecisive() : "Expected Indecisive";
    }

    public static void testJDecisionMap() {
        JDecision<String, String, Integer> dec = JDecision.pure(5);
        JDecision<String, String, String> mapped = dec.<Integer, String>map(x -> "v=" + x);
        assert mapped.isIndecisive() : "Expected Indecisive after map";
    }

    public static void testJDecisionAcceptReturn() {
        JDecision<String, String, Integer> dec = JDecision.acceptReturn(42, "e1");
        assert dec.isAccepted() : "Expected Accepted";
    }

    // -- JCommandMessage tests --

    public static void testJCommandMessage() {
        Instant now = Instant.now();
        JCommandMessage<String> cmd = JCommandMessage.of("cmd-1", now, "agg-1", "do-something");
        assert cmd.id().equals("cmd-1") : "Expected id 'cmd-1'";
        assert cmd.address().equals("agg-1") : "Expected address 'agg-1'";
        assert cmd.payload().equals("do-something") : "Expected payload 'do-something'";
        assert cmd.time().equals(now) : "Expected correct time";
    }

    // -- JAppResult tests --

    public static void testJAppResultAccept() {
        JAppResult<String, String, String> result = JAppResult.accept("event1");
        assert result.decision().isAccepted() : "Expected Accepted decision";
        assert result.notifications().isEmpty() : "Expected no notifications";
    }

    public static void testJAppResultReject() {
        JAppResult<String, String, String> result = JAppResult.reject("bad-input");
        assert result.decision().isRejected() : "Expected Rejected decision";
    }

    public static void testJAppResultPublish() {
        List<String> notifs = Arrays.asList("notif1", "notif2");
        JAppResult<String, String, String> result = JAppResult.publish(notifs);
        assert result.decision().isIndecisive() : "Expected Indecisive decision";
        assert result.notifications().size() == 2 : "Expected 2 notifications";
    }

    // -- JCommandHandler tests --

    public static void testJCommandHandler() {
        JCommandHandler<String, Integer, String, String, String> handler =
            JCommandHandler.create(ctx -> {
                String command = ctx.command();
                Integer state = ctx.state();
                if ("increment".equals(command)) {
                    return JAppResult.accept("incremented");
                } else {
                    return JAppResult.reject("unknown command: " + command);
                }
            });

        assert handler != null : "Handler should not be null";
    }

    // -- JCodec tests --

    public static void testJCodec() {
        JCodec<Integer> codec = JCodec.of(
            i -> Integer.toString(i),
            s -> Integer.parseInt(s)
        );
        assert codec.encode(42).equals("42") : "Expected '42'";
        assert codec.decode("99") == 99 : "Expected 99";
    }

    // -- JPGSchema tests --

    public static void testJPGSchemaEventsourcing() {
        List<String> ddl = JPGSchema.eventsourcing("myapp");
        assert !ddl.isEmpty() : "Expected DDL statements";
        String allDDL = String.join("\n", ddl);
        assert allDDL.contains("myapp_journal") : "Expected journal table";
        assert allDDL.contains("myapp_outbox") : "Expected outbox table";
        assert allDDL.contains("myapp_commands") : "Expected commands table";
        assert allDDL.contains("myapp_snapshots") : "Expected snapshots table";
    }

    public static void testJPGSchemaCqrs() {
        List<String> ddl = JPGSchema.cqrs("myapp");
        assert !ddl.isEmpty() : "Expected DDL statements";
        String allDDL = String.join("\n", ddl);
        assert allDDL.contains("myapp_states") : "Expected states table";
    }

    // -- EdomataRuntime tests --

    public static void testEdomataRuntime() {
        EdomataRuntime runtime = EdomataRuntime.create();
        assert runtime != null : "Runtime should not be null";
        runtime.close();
    }

    // -- Run all tests --

    public static void main(String[] args) {
        System.out.println("Running Java API integration tests...");

        testJEitherRight();
        testJEitherLeft();
        testJEitherFold();
        testJDecisionAccept();
        testJDecisionReject();
        testJDecisionPure();
        testJDecisionMap();
        testJDecisionAcceptReturn();
        testJCommandMessage();
        testJAppResultAccept();
        testJAppResultReject();
        testJAppResultPublish();
        testJCommandHandler();
        testJCodec();
        testJPGSchemaEventsourcing();
        testJPGSchemaCqrs();
        testEdomataRuntime();

        System.out.println("All 17 Java integration tests passed!");
    }
}
