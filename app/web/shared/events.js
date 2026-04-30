// HOS Event Bus — replaces monkey-patching pattern
// Load before app.js, after crypto.js
window.hos = window.hos || {};

(function() {
  var listeners = {};

  hos.on = function(event, fn, priority) {
    // priority: lower = runs first. Default 100.
    if (!listeners[event]) listeners[event] = [];
    listeners[event].push({ fn: fn, priority: priority || 100 });
    listeners[event].sort(function(a, b) { return a.priority - b.priority; });
  };

  hos.off = function(event, fn) {
    if (!listeners[event]) return;
    listeners[event] = listeners[event].filter(function(l) { return l.fn !== fn; });
  };

  hos.emit = function(event, data) {
    if (!listeners[event]) return;
    for (var i = 0; i < listeners[event].length; i++) {
      try {
        var result = listeners[event][i].fn(data);
        if (result === false) break; // stop propagation
      } catch (e) {
        console.error('[hos.emit] Error in ' + event + ' handler:', e);
      }
    }
  };

  // Convenience: emit and collect results
  hos.gather = function(event, data) {
    var results = [];
    if (!listeners[event]) return results;
    for (var i = 0; i < listeners[event].length; i++) {
      try {
        results.push(listeners[event][i].fn(data));
      } catch (e) {
        console.error('[hos.gather] Error in ' + event + ' handler:', e);
      }
    }
    return results;
  };

  // Debug: list all registered events
  hos.events = function() {
    var summary = {};
    for (var key in listeners) {
      summary[key] = listeners[key].length;
    }
    return summary;
  };
})();
