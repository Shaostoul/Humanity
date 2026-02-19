USE NS project_universe;
USE DB entities;

DEFINE TABLE ai_characters SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD character_id    ON TABLE ai_characters TYPE string;
DEFINE FIELD name            ON TABLE ai_characters TYPE string;
DEFINE FIELD type            ON TABLE ai_characters TYPE string;
DEFINE FIELD capabilities    ON TABLE ai_characters TYPE array;
DEFINE FIELD applications    ON TABLE ai_characters TYPE array;

DEFINE INDEX unique_ai_character_id ON TABLE ai_characters COLUMNS character_id UNIQUE;

-- Example AI characters
CREATE ai_characters:character_001 SET
  character_id = "character_001",
  name = "Chatbot",
  type = "Conversational AI",
  capabilities = ["Natural Language Processing", "Contextual Understanding"],
  applications = ["Customer Support", "Virtual Assistance"];

CREATE ai_characters:character_002 SET
  character_id = "character_002",
  name = "Virtual Assistant",
  type = "Personal AI",
  capabilities = ["Task Automation", "Voice Recognition"],
  applications = ["Home Automation", "Personal Scheduling"];
