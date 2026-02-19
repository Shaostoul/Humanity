USE NS project_universe;
USE DB health;

DEFINE TABLE health_systems SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE health_systems TYPE string;
DEFINE FIELD description     ON TABLE health_systems TYPE string;
DEFINE FIELD type            ON TABLE health_systems TYPE string;
DEFINE FIELD components      ON TABLE health_systems TYPE array;
DEFINE FIELD applications    ON TABLE health_systems TYPE array;
DEFINE FIELD related_fields  ON TABLE health_systems TYPE array;

DEFINE INDEX unique_health_systems_name ON TABLE health_systems COLUMNS name UNIQUE;

-- Example health systems
CREATE health_systems:public_health SET
  name = "Public Health",
  description = "A system focused on the health of populations through prevention, promotion, and protection activities",
  type = "Preventive",
  components = ["Epidemiology", "Health Education"],
  applications = ["Disease Prevention", "Health Promotion"],
  related_fields = ["medicine:epidemiology", "medicine:health_education"];

CREATE health_systems:primary_care SET
  name = "Primary Care",
  description = "A system providing first contact and continuous care for patients",
  type = "Clinical",
  components = ["General Practitioners", "Nurses"],
  applications = ["Routine Check-ups", "Chronic Disease Management"],
  related_fields = ["medicine:family_medicine", "medicine:internal_medicine"];

CREATE health_systems:emergency_medicine SET
  name = "Emergency Medicine",
  description = "A system providing immediate care for acute illnesses and injuries",
  type = "Acute",
  components = ["Emergency Physicians", "Paramedics"],
  applications = ["Trauma Care", "Acute Illness Management"],
  related_fields = ["medicine:trauma_surgery", "medicine:critical_care"];

CREATE health_systems:mental_health SET
  name = "Mental Health",
  description = "A system focused on the psychological well-being of individuals",
  type = "Supportive",
  components = ["Psychiatrists", "Therapists"],
  applications = ["Counseling", "Psychotherapy"],
  related_fields = ["medicine:psychiatry", "medicine:psychology"];
