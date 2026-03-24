USE NS project_universe;
USE DB engineering;

DEFINE TABLE electrical_engineering SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE electrical_engineering TYPE string;
DEFINE FIELD description     ON TABLE electrical_engineering TYPE string;
DEFINE FIELD type            ON TABLE electrical_engineering TYPE string;
DEFINE FIELD components      ON TABLE electrical_engineering TYPE array;
DEFINE FIELD applications    ON TABLE electrical_engineering TYPE array;
DEFINE FIELD related_fields  ON TABLE electrical_engineering TYPE array;

DEFINE INDEX unique_electrical_engineering_name ON TABLE electrical_engineering COLUMNS name UNIQUE;

-- Example electrical engineering concepts
CREATE electrical_engineering:power_systems SET
  name = "Power Systems",
  description = "Systems for generating, transmitting, and distributing electrical power",
  type = "Field",
  components = ["Generators", "Transformers"],
  applications = ["Electricity Distribution", "Renewable Energy"],
  related_fields = ["engineering:energy", "engineering:control_systems"];

CREATE electrical_engineering:control_systems SET
  name = "Control Systems",
  description = "Systems that manage, command, direct, or regulate the behavior of other devices or systems",
  type = "Field",
  components = ["Sensors", "Actuators"],
  applications = ["Automation", "Robotics"],
  related_fields = ["engineering:automation", "engineering:robotics"];

CREATE electrical_engineering:electronics SET
  name = "Electronics",
  description = "The study and use of devices that control the flow of electrons or other electrically charged particles",
  type = "Field",
  components = ["Semiconductors", "Circuits"],
  applications = ["Consumer Electronics", "Telecommunications"],
  related_fields = ["engineering:telecommunications", "engineering:computer_science"];
