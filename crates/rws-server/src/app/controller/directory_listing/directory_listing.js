(function () {
  var filter = document.getElementById('filter');
  if (!filter) { return; }
  filter.addEventListener('input', function () {
    var q = filter.value.toLowerCase();
    var rows = document.querySelectorAll('#rows tr.entry:not(.parent)');
    for (var i = 0; i < rows.length; i++) {
      var row = rows[i];
      row.style.display = row.dataset.name.indexOf(q) !== -1 ? '' : 'none';
    }
  });
})();
