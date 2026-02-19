USE NS project_universe;
USE DB humanities;

DEFINE TABLE anthropology SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE anthropology TYPE string;
DEFINE FIELD description     ON TABLE anthropology TYPE string;
DEFINE FIELD type            ON TABLE anthropology TYPE string;
DEFINE FIELD theories        ON TABLE anthropology TYPE array;
DEFINE FIELD applications    ON TABLE anthropology TYPE array;
DEFINE FIELD limitations     ON TABLE anthropology TYPE array;
DEFINE FIELD related_fields  ON TABLE anthropology TYPE array;

DEFINE INDEX unique_anthropology_name ON TABLE anthropology COLUMNS name UNIQUE;

-- Example anthropological concepts
CREATE anthropology:cultural_relativism SET
  name = "Cultural Relativism",
  description = "The principle of regarding the beliefs, values, and practices of a culture from the viewpoint of that culture itself",
  type = "Theory",
  theories = ["Cultural Understanding", "Ethnocentrism Avoidance"],
  applications = ["Cross-Cultural Studies", "Ethnographic Research"],
  limitations = ["Moral Relativism"],
  related_fields = ["humanities:sociology", "humanities:philosophy"];

CREATE anthropology:structuralism SET
  name = "Structuralism",
  description = "An anthropological theory that elements of human culture must be understood in terms of their relationship to a larger, overarching system or structure",
  type = "Theory",
  theories = ["Binary Oppositions", "Cultural Structures"],
  applications = ["Linguistic Analysis", "Cultural Studies"],
  limitations = ["Overemphasis on Structure"],
  related_fields = ["humanities:linguistics", "humanities:psychology"];

CREATE anthropology:ethnography SET
  name = "Ethnography",
  description = "The systematic study of people and cultures from the point of view of the subject",
  type = "Methodology",
  theories = ["Participant Observation", "Fieldwork"],
  applications = ["Cultural Documentation", "Social Research"],
  limitations = ["Subjectivity"],
  related_fields = ["humanities:sociology", "humanities:history"];
