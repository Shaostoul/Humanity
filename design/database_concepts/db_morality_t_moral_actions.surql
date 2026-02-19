USE NS project_universe;
USE DB game;

DEFINE TABLE moral_actions SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD action_name     ON TABLE moral_actions TYPE string;
DEFINE FIELD description     ON TABLE moral_actions TYPE string;
DEFINE FIELD intention       ON TABLE moral_actions TYPE string;
DEFINE FIELD consequence     ON TABLE moral_actions TYPE string;
DEFINE FIELD related_sin     ON TABLE moral_actions TYPE record(sins);
DEFINE FIELD related_virtue  ON TABLE moral_actions TYPE record(virtues);

DEFINE INDEX unique_action_name ON TABLE moral_actions COLUMNS action_name UNIQUE;

-- Actions related to Pride vs Humility
CREATE moral_actions:truth_telling SET
  action_name = "Truth Telling",
  description = "Being honest and transparent in communication.",
  intention = "To maintain trust and integrity",
  consequence = "Builds trust and promotes understanding",
  related_sin = sins:pride,
  related_virtue = virtues:humility;

CREATE moral_actions:modest_behavior SET
  action_name = "Modest Behavior",
  description = "Acting with humility and restraint.",
  intention = "To avoid arrogance and promote humility",
  consequence = "Fosters respect and genuine relationships",
  related_sin = sins:pride,
  related_virtue = virtues:humility;

CREATE moral_actions:accepting_criticism SET
  action_name = "Accepting Criticism",
  description = "Openly receiving and considering feedback from others.",
  intention = "To improve oneself through constructive feedback",
  consequence = "Personal growth and better relationships",
  related_sin = sins:pride,
  related_virtue = virtues:humility;

-- Actions related to Greed vs Charity
CREATE moral_actions:charitable_donation SET
  action_name = "Charitable Donation",
  description = "Giving money or resources to help those in need.",
  intention = "To help others and reduce suffering",
  consequence = "Improved well-being of recipients and community",
  related_sin = sins:greed,
  related_virtue = virtues:charity;

CREATE moral_actions:volunteering SET
  action_name = "Volunteering",
  description = "Offering time and skills to help others without payment.",
  intention = "To contribute to community welfare",
  consequence = "Strengthens community bonds and personal fulfillment",
  related_sin = sins:greed,
  related_virtue = virtues:charity;

CREATE moral_actions:sharing_resources SET
  action_name = "Sharing Resources",
  description = "Willingly sharing possessions or opportunities with others.",
  intention = "To promote equality and community",
  consequence = "Creates a culture of generosity and mutual support",
  related_sin = sins:greed,
  related_virtue = virtues:charity;

-- Actions related to Lust vs Chastity
CREATE moral_actions:chaste_conduct SET
  action_name = "Chaste Conduct",
  description = "Maintaining purity in thoughts and actions.",
  intention = "To respect self and others",
  consequence = "Builds healthy relationships and self-respect",
  related_sin = sins:lust,
  related_virtue = virtues:chastity;

CREATE moral_actions:respectful_relationships SET
  action_name = "Respectful Relationships",
  description = "Maintaining appropriate boundaries in relationships.",
  intention = "To honor personal and social boundaries",
  consequence = "Healthy, lasting relationships",
  related_sin = sins:lust,
  related_virtue = virtues:chastity;

CREATE moral_actions:mindful_media_consumption SET
  action_name = "Mindful Media Consumption",
  description = "Being selective about media content and its influence.",
  intention = "To maintain mental purity and focus",
  consequence = "Better mental health and clearer thinking",
  related_sin = sins:lust,
  related_virtue = virtues:chastity;

-- Actions related to Envy vs Kindness
CREATE moral_actions:compassionate_action SET
  action_name = "Compassionate Action",
  description = "Showing kindness and understanding to others.",
  intention = "To support and uplift others",
  consequence = "Creates positive relationships and community",
  related_sin = sins:envy,
  related_virtue = virtues:kindness;

CREATE moral_actions:celebrating_others SET
  action_name = "Celebrating Others",
  description = "Genuinely celebrating others' achievements and successes.",
  intention = "To foster positive community spirit",
  consequence = "Strengthens relationships and reduces envy",
  related_sin = sins:envy,
  related_virtue = virtues:kindness;

CREATE moral_actions:offering_support SET
  action_name = "Offering Support",
  description = "Providing help and encouragement to those in need.",
  intention = "To build others up",
  consequence = "Creates a supportive community environment",
  related_sin = sins:envy,
  related_virtue = virtues:kindness;

-- Actions related to Gluttony vs Temperance
CREATE moral_actions:self_control SET
  action_name = "Self-Control",
  description = "Exercising restraint in desires and impulses.",
  intention = "To maintain personal integrity",
  consequence = "Better decision-making and character development",
  related_sin = sins:gluttony,
  related_virtue = virtues:temperance;

CREATE moral_actions:mindful_eating SET
  action_name = "Mindful Eating",
  description = "Eating with awareness and moderation.",
  intention = "To maintain physical and mental health",
  consequence = "Better health and relationship with food",
  related_sin = sins:gluttony,
  related_virtue = virtues:temperance;

CREATE moral_actions:balanced_lifestyle SET
  action_name = "Balanced Lifestyle",
  description = "Maintaining moderation in all aspects of life.",
  intention = "To achieve sustainable well-being",
  consequence = "Long-term health and happiness",
  related_sin = sins:gluttony,
  related_virtue = virtues:temperance;

-- Actions related to Wrath vs Patience
CREATE moral_actions:peaceful_resolution SET
  action_name = "Peaceful Resolution",
  description = "Handling conflicts with calm and understanding.",
  intention = "To resolve conflicts without violence",
  consequence = "Promotes harmony and prevents escalation",
  related_sin = sins:wrath,
  related_virtue = virtues:patience;

CREATE moral_actions:active_listening SET
  action_name = "Active Listening",
  description = "Fully engaging with others' perspectives without interruption.",
  intention = "To understand and respect others",
  consequence = "Better communication and reduced conflict",
  related_sin = sins:wrath,
  related_virtue = virtues:patience;

CREATE moral_actions:forgiveness_practice SET
  action_name = "Forgiveness Practice",
  description = "Choosing to forgive and let go of resentment.",
  intention = "To maintain inner peace and relationships",
  consequence = "Emotional healing and reconciliation",
  related_sin = sins:wrath,
  related_virtue = virtues:patience;

-- Actions related to Sloth vs Diligence
CREATE moral_actions:dedicated_work SET
  action_name = "Dedicated Work",
  description = "Consistently applying effort to tasks and responsibilities.",
  intention = "To fulfill obligations and develop skills",
  consequence = "Achievement of goals and personal growth",
  related_sin = sins:sloth,
  related_virtue = virtues:diligence;

CREATE moral_actions:habit_formation SET
  action_name = "Habit Formation",
  description = "Building positive routines and consistent practices.",
  intention = "To develop lasting positive behaviors",
  consequence = "Improved productivity and character",
  related_sin = sins:sloth,
  related_virtue = virtues:diligence;

CREATE moral_actions:proactive_initiative SET
  action_name = "Proactive Initiative",
  description = "Taking initiative to address needs and opportunities.",
  intention = "To contribute actively to community and personal growth",
  consequence = "Positive change and personal development",
  related_sin = sins:sloth,
  related_virtue = virtues:diligence; 