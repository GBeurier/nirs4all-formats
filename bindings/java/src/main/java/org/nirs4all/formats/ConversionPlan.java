package org.nirs4all.formats;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * Steps that make records writable for a target, from {@link Nirs4allFormats#planConversion}.
 *
 * <p>The plan computes nothing. Each numerical step names the nirs4all-methods catalog method
 * that computes it ({@link Step#methodId()}); run those methods yourself (for example with the
 * nirs4all-methods library), or export records that are already converted.
 */
public final class ConversionPlan {
    /**
     * One step, in execution order.
     *
     * @param step {@code signal_conversion}, {@code resample}, {@code axis_unit_conversion},
     *     {@code select_points}, {@code declare_signal_type} or {@code reflectance_from_references}
     * @param methodId nirs4all-methods catalog method, or {@code null} for selections and
     *     declarations
     * @param params method parameters
     * @param description what the step does
     * @param notes limits of the step
     */
    public record Step(String step, String methodId, ObjectNode params, String description, List<String> notes) {}

    /**
     * A choice offered by a question.
     *
     * @param value option value to set
     * @param label description
     */
    public record Choice(String value, String label) {}

    /**
     * Information the records do not hold; answer it through {@link PlanOptions} and plan again.
     *
     * @param id option that answers it, such as {@code source_scale}
     * @param prompt question
     * @param choices accepted answers
     * @param why why it is asked
     */
    public record Question(String id, String prompt, List<Choice> choices, String why) {}

    /**
     * Why the records cannot be converted for the target.
     *
     * @param code stable identifier, such as {@code raman_shift}
     * @param reason explanation
     * @param fix what would make it possible
     */
    public record Refusal(String code, String reason, String fix) {}

    private final ObjectNode json;
    private final List<Step> steps = new ArrayList<>();
    private final List<Question> questions = new ArrayList<>();
    private final List<Refusal> refusals = new ArrayList<>();

    ConversionPlan(ObjectNode json) {
        this.json = json;
        json.path("steps").forEach(s -> steps.add(new Step(
                Json.text(s, "step"),
                Json.text(s, "method_id"),
                s.get("params") instanceof ObjectNode params ? params.deepCopy() : Json.object(),
                Json.text(s, "description"),
                Json.strings(s.get("notes")))));
        json.path("questions").forEach(q -> {
            List<Choice> choices = new ArrayList<>();
            q.path("choices").forEach(c -> choices.add(new Choice(Json.text(c, "value"), Json.text(c, "label"))));
            questions.add(new Question(
                    Json.text(q, "id"), Json.text(q, "prompt"), Collections.unmodifiableList(choices), Json.text(q, "why")));
        });
        json.path("refusals").forEach(r ->
                refusals.add(new Refusal(Json.text(r, "code"), Json.text(r, "reason"), Json.text(r, "fix"))));
    }

    /** {@code ready}, {@code needs_answers}, {@code refused} or {@code unchanged}. */
    public String status() {
        return Json.text(json, "status");
    }

    /** Writer format of the target, or {@code null} when planning the options alone. */
    public String format() {
        return Json.text(json, "format");
    }

    /** Signal the steps start from. */
    public String sourceSignal() {
        return Json.text(json, "source_signal");
    }

    /** Steps to run, in order. */
    public List<Step> steps() {
        return Collections.unmodifiableList(steps);
    }

    /** Questions to answer through {@link PlanOptions}. */
    public List<Question> questions() {
        return Collections.unmodifiableList(questions);
    }

    /** Reasons the conversion is refused. */
    public List<Refusal> refusals() {
        return Collections.unmodifiableList(refusals);
    }

    /** The complete plan JSON (output signal, candidates, grid, notes), a copy. */
    public JsonNode json() {
        return json.deepCopy();
    }

    @Override
    public String toString() {
        return "ConversionPlan[status=" + status() + ", steps=" + steps.size() + "]";
    }
}
