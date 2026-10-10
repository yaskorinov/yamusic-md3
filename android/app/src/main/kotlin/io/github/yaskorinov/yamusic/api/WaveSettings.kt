package io.github.yaskorinov.yamusic.api

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

const val WAVE_SEED = "user:onyourwave"

class WaveItem(val label: String, val seed: String)

/** Группа настроек волны: context (занятие), diversity (характер), moodEnergy (настроение), language (язык). */
class WaveGroup(val key: String, val title: String, val items: List<WaveItem>)

/** Если /rotor/wave/settings недоступен. Сиды — как в ответе API. */
val FALLBACK_WAVE_GROUPS = listOf(
    WaveGroup(
        "context", "Занятие",
        listOf(
            WaveItem("Танцую", "genre:dance"),
            WaveItem("Просыпаюсь", "activity:wake-up"),
            WaveItem("В дороге", "activity:road-trip"),
            WaveItem("Работаю", "activity:work-background"),
            WaveItem("Тренируюсь", "activity:workout"),
            WaveItem("Отдыхаю", "mood:relaxed"),
            WaveItem("Засыпаю", "activity:fall-asleep"),
        ),
    ),
    WaveGroup(
        "diversity", "Характер",
        listOf(
            WaveItem("Любимое", "settingDiversity:favorite"),
            WaveItem("Незнакомое", "settingDiversity:discover"),
            WaveItem("Популярное", "settingDiversity:popular"),
        ),
    ),
    WaveGroup(
        "moodEnergy", "Настроение",
        listOf(
            WaveItem("Бодрое", "settingMoodEnergy:active"),
            WaveItem("Весёлое", "settingMoodEnergy:fun"),
            WaveItem("Спокойное", "settingMoodEnergy:calm"),
            WaveItem("Грустное", "settingMoodEnergy:sad"),
        ),
    ),
    WaveGroup(
        "language", "Язык",
        listOf(
            WaveItem("Русский", "settingLanguage:russian"),
            WaveItem("Иностранный", "settingLanguage:not-russian"),
            WaveItem("Без слов", "settingLanguage:without-words"),
        ),
    ),
)

private fun JsonElement?.obj(key: String): JsonObject? = (this as? JsonObject)?.get(key) as? JsonObject
private fun JsonElement?.list(key: String): List<JsonElement> = ((this as? JsonObject)?.get(key) as? JsonArray).orEmpty()
private fun JsonElement?.text(key: String): String =
    ((this as? JsonObject)?.get(key) as? JsonPrimitive)?.takeIf { it.isString }?.content.orEmpty()
private fun JsonElement?.flag(key: String): Boolean = ((this as? JsonObject)?.get(key) as? JsonPrimitive)?.content == "true"

/** Разбор ответа /rotor/wave/settings: занятия — в блоке contexts, остальное — в settingRestrictions. */
internal fun waveGroups(settings: JsonObject): List<WaveGroup> {
    val found = HashMap<String, WaveGroup>()
    val contexts = settings.list("blocks").filter { it.text("type") == "contexts" }.flatMap { it.list("items") }.mapNotNull { item ->
        val station = item.obj("station") ?: item as? JsonObject
        val id = station.obj("id")
        val name = station.text("name")
        if (name.isEmpty() || id.text("type").isEmpty()) null else WaveItem(name, "${id.text("type")}:${id.text("tag")}")
    }
    if (contexts.isNotEmpty()) found["context"] = WaveGroup("context", "Занятие", contexts)

    val restrictions = settings.obj("settingRestrictions")
    for (fallback in FALLBACK_WAVE_GROUPS) {
        val items = restrictions.obj(fallback.key).list("possibleValues").mapNotNull { value ->
            val seed = value.text("serializedSeed")
            if (seed.isEmpty() || value.flag("unspecified")) null else WaveItem(value.text("name"), seed)
        }
        if (items.isNotEmpty()) found[fallback.key] = WaveGroup(fallback.key, fallback.title, items)
    }
    return FALLBACK_WAVE_GROUPS.map { found[it.key] ?: it }
}
