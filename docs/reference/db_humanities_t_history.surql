USE NS project_universe;
USE DB humanities;

DEFINE TABLE history SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE history TYPE string;
DEFINE FIELD description     ON TABLE history TYPE string;
DEFINE FIELD period          ON TABLE history TYPE string;
DEFINE FIELD events          ON TABLE history TYPE array;
DEFINE FIELD significance    ON TABLE history TYPE array;
DEFINE FIELD limitations     ON TABLE history TYPE array;
DEFINE FIELD related_fields  ON TABLE history TYPE array;

DEFINE INDEX unique_history_name ON TABLE history COLUMNS name UNIQUE;

-- Example historical periods
CREATE history:renaissance SET
  name = "Renaissance",
  description = "A period in European history marking the transition from the Middle Ages to modernity",
  period = "14th to 17th century",
  events = ["Cultural Revival", "Scientific Advancements"],
  significance = ["Birth of Humanism", "Artistic Flourishing"],
  limitations = ["Eurocentric Focus"],
  related_fields = ["humanities:art", "humanities:philosophy"];

CREATE history:industrial_revolution SET
  name = "Industrial Revolution",
  description = "The transition to new manufacturing processes in Europe and the United States",
  period = "18th to 19th century",
  events = ["Mechanization", "Urbanization"],
  significance = ["Economic Growth", "Social Changes"],
  limitations = ["Environmental Impact"],
  related_fields = ["humanities:economics", "humanities:sociology"];

CREATE history:world_war_ii SET
  name = "World War II",
  description = "A global war that lasted from 1939 to 1945, involving most of the world's nations",
  period = "1939 to 1945",
  events = ["Allied Victory", "Holocaust"],
  significance = ["Geopolitical Shifts", "Technological Innovations"],
  limitations = ["Destruction and Loss"],
  related_fields = ["humanities:politics", "humanities:law"];
