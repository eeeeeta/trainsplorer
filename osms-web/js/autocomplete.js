@import '../node_modules/accessible-autocomplete/dist/accessible-autocomplete.min.js';
function suggest(query, populateResults) {
	fetch("/station_suggestions?query=" + encodeURIComponent(query))
		.then(function(resp) { return resp.json(); })
		.then(function(data) {
			populateResults(data.suggestions);
		})
		.catch(function(err) {
			console.error("Request failed: " + err);
		});
}
function inputTemplate(result) {
	if (result) {
		return result.code;
	}
}
function suggestionTemplate(result) {
	return "<strong>" + result.name + "</strong> <small><i>(" + result.code_type + " code " + result.code + ")</i></small>";
}
window.onload = function() {
	var input = document.querySelector("#ts-tiploc");
	var value = input.value ? input.value : "";
	var auto = document.querySelector("#ts-tiploc-autocomplete");
	auto.innerHTML = "";
	var verbiage = document.querySelector("#ts-tiploc-verbiage");
	verbiage.innerHTML = "Start typing a station name, <abbr title=\"Timing Point Location (a code used to identify locations in schedules)\">TIPLOC</abbr>, or <abbr title=\"Customer Reservation System (a code used to identify stations)\">CRS</abbr> code.<br>Examples: 'STPANCI', 'CLJ', 'waterloo'";
	accessibleAutocomplete({
		element: auto,
		id: 'ts-tiploc',
		name: 'ts-tiploc',
		required: true,
		defaultValue: value,
		minLength: 3,
		autoselect: true,
		confirmOnBlur: false,
		templates: {
			suggestion: suggestionTemplate,
			inputValue: inputTemplate,
		},
		tNoResults: function() {
			"No stations found"
		},
		source: suggest
	});
};
