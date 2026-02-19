USE NS project_universe;
USE DB mathematics;

DEFINE TABLE statistical_methods SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE statistical_methods TYPE string;
DEFINE FIELD description     ON TABLE statistical_methods TYPE string;
DEFINE FIELD formula         ON TABLE statistical_methods TYPE string;
DEFINE FIELD variables       ON TABLE statistical_methods TYPE array;
DEFINE FIELD assumptions     ON TABLE statistical_methods TYPE array;
DEFINE FIELD applications    ON TABLE statistical_methods TYPE array;
DEFINE FIELD example_use     ON TABLE statistical_methods TYPE string;

DEFINE INDEX unique_statistical_method ON TABLE statistical_methods COLUMNS name UNIQUE;

CREATE statistical_methods:normal_distribution SET
  name = "Normal Distribution",
  description = "Continuous probability distribution symmetric about the mean.",
  formula = "(1 / (σ√(2π))) * e^(-(x - μ)² / (2σ²))",
  variables = ["μ (mean)", "σ (standard deviation)", "x (value)"],
  assumptions = ["Symmetrical distribution", "Continuous variable", "Infinite population"],
  applications = ["Crew Population Studies", "Habitat Design Standards"],
  example_use = "Modeling height distribution of colonists for habitat design.";

CREATE statistical_methods:binomial_distribution SET
  name = "Binomial Distribution",
  description = "Discrete probability distribution of the number of successes in a sequence of independent experiments.",
  formula = "P(X=k) = C(n,k) * p^k * (1-p)^(n-k)",
  variables = ["n (trials)", "k (successes)", "p (probability of success)"],
  assumptions = ["Independent trials", "Fixed probability", "Discrete outcomes"],
  applications = ["Mission Success Probability", "Equipment Reliability Testing"],
  example_use = "Calculating probability of successful equipment operations.";

CREATE statistical_methods:poisson_distribution SET
  name = "Poisson Distribution",
  description = "Discrete probability distribution expressing the probability of events occurring in a fixed interval.",
  formula = "P(X=k) = (λ^k * e^(-λ)) / k!",
  variables = ["λ (average rate)", "k (number of events)"],
  assumptions = ["Events are independent", "Average rate is constant", "Events cannot occur simultaneously"],
  applications = ["Meteor Shower Prediction", "Equipment Failure Analysis"],
  example_use = "Predicting frequency of meteor impacts on space stations.";

CREATE statistical_methods:linear_regression SET
  name = "Linear Regression",
  description = "Statistical method for modeling the relationship between a dependent variable and one or more independent variables.",
  formula = "y = mx + b",
  variables = ["y (dependent variable)", "x (independent variable)", "m (slope)", "b (y-intercept)"],
  assumptions = ["Linear relationship", "Independent observations", "Normal distribution of errors"],
  applications = ["Resource Use Forecasting", "Fuel Consumption Modeling", "Economic Modeling"],
  example_use = "Predicting resource consumption patterns in space habitats.";

CREATE statistical_methods:anova SET
  name = "Analysis of Variance (ANOVA)",
  description = "Statistical method for analyzing differences between group means.",
  formula = "F = MSB/MSW",
  variables = ["F (F-statistic)", "MSB (mean square between)", "MSW (mean square within)"],
  assumptions = ["Normal distribution", "Equal variances", "Independent samples"],
  applications = ["Experimental Results Evaluation", "Agricultural Productivity Tests"],
  example_use = "Comparing effectiveness of different agricultural methods in space habitats.";

CREATE statistical_methods:chi_squared SET
  name = "Chi-Squared Test",
  description = "Statistical test for categorical data to determine if observed frequencies differ from expected frequencies.",
  formula = "χ² = Σ((O-E)²/E)",
  variables = ["O (observed frequency)", "E (expected frequency)"],
  assumptions = ["Independent observations", "Expected frequency > 5", "Categorical data"],
  applications = ["Player Decision Analytics", "Colonist Preferences Analysis"],
  example_use = "Analyzing colonist preferences for different habitat designs.";

CREATE statistical_methods:bayesian_inference SET
  name = "Bayesian Inference",
  description = "Statistical method that updates probability estimates as new information becomes available.",
  formula = "P(A|B) = P(B|A) * P(A) / P(B)",
  variables = ["P(A|B) (posterior)", "P(B|A) (likelihood)", "P(A) (prior)", "P(B) (evidence)"],
  assumptions = ["Prior knowledge available", "Conditional independence", "Proper prior distribution"],
  applications = ["Real-time Mission Probability Updates", "Adaptive Decision-making Systems"],
  example_use = "Updating mission success probabilities based on real-time data.";
