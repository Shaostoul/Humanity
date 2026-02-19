USE NS project_universe;
USE DB technology;

DEFINE TABLE ai_systems SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE ai_systems TYPE string;
DEFINE FIELD description     ON TABLE ai_systems TYPE string;
DEFINE FIELD type            ON TABLE ai_systems TYPE string;
DEFINE FIELD algorithms      ON TABLE ai_systems TYPE array;
DEFINE FIELD capabilities    ON TABLE ai_systems TYPE array;
DEFINE FIELD applications    ON TABLE ai_systems TYPE array;
DEFINE FIELD limitations     ON TABLE ai_systems TYPE array;
DEFINE FIELD related_systems ON TABLE ai_systems TYPE array;

DEFINE INDEX unique_ai_system_name ON TABLE ai_systems COLUMNS name UNIQUE;

-- Example AI systems
CREATE ai_systems:machine_learning SET
  name = "Machine Learning",
  description = "AI system that learns from data to make predictions or decisions",
  type = "Supervised/Unsupervised",
  algorithms = ["Neural Networks", "Decision Trees", "Support Vector Machines"],
  capabilities = ["Pattern Recognition", "Predictive Analytics", "Classification"],
  applications = ["Image Recognition", "Fraud Detection", "Recommendation Systems"],
  limitations = ["Data Dependency", "Overfitting"],
  related_systems = ["technology:data_processing", "technology:cloud_computing"];

CREATE ai_systems:natural_language_processing SET
  name = "Natural Language Processing",
  description = "AI system for understanding and generating human language",
  type = "Linguistic",
  algorithms = ["Tokenization", "Sentiment Analysis", "Language Modeling"],
  capabilities = ["Text Analysis", "Language Translation", "Chatbots"],
  applications = ["Customer Support", "Content Moderation", "Voice Assistants"],
  limitations = ["Context Understanding", "Ambiguity"],
  related_systems = ["technology:communication", "technology:cloud_computing"];

CREATE ai_systems:computer_vision SET
  name = "Computer Vision",
  description = "AI system for interpreting and understanding visual information",
  type = "Visual",
  algorithms = ["Convolutional Neural Networks", "Image Segmentation", "Object Detection"],
  capabilities = ["Image Recognition", "Video Analysis", "Facial Recognition"],
  applications = ["Surveillance", "Autonomous Vehicles", "Medical Imaging"],
  limitations = ["Lighting Conditions", "Occlusion"],
  related_systems = ["technology:sensors", "technology:robotics"];

CREATE ai_systems:robotics_ai SET
  name = "Robotics AI",
  description = "AI system for controlling and automating robotic systems",
  type = "Control",
  algorithms = ["Path Planning", "Motion Control", "Sensor Fusion"],
  capabilities = ["Autonomous Navigation", "Task Automation", "Environment Interaction"],
  applications = ["Manufacturing", "Logistics", "Space Exploration"],
  limitations = ["Complex Environments", "Real-time Processing"],
  related_systems = ["technology:robotics", "technology:control"];
