
function http_get(theUrl, callback) {
	var xmlHttp = new XMLHttpRequest();
	xmlHttp.onreadystatechange = function() { 
		if (xmlHttp.readyState == 4 && xmlHttp.status == 200)
			callback(xmlHttp.responseText);
	};
	xmlHttp.open("GET", theUrl, true); // true for asynchronous 
	xmlHttp.send(null);
}
function on_each_way(feature, layer) {
	if (feature.properties && feature.properties.p1 && feature.properties.p2) {
		layer.bindPopup("Link " + feature.properties.p1 + " <-> " +
			feature.properties.p2);
	}
}
function on_each_station(feature, layer) {
	if (feature.properties && feature.properties.nr_ref) {
		layer.bindPopup("Station " + feature.properties.nr_ref);
	}
}
function update_map(map) {
	var bounds = map.getBounds();
	var c1 = bounds.getNorthWest();
	var c2 = bounds.getSouthEast();
	let qs = "?xmin=" + c1.lng + "&xmax=" + c2.lng +
		"&ymin=" + c1.lat + "&ymax=" + c2.lat;
	console.log("Requesting " + qs);
	http_get("/geo/ways" + qs, function(data) {
		console.log("Got some data!");
		data = JSON.parse(data);
		if (map.json_layers.ways) {
			map.json_layers.ways.remove();
		}
		map.json_layers.ways = L.geoJSON(data, {
			onEachFeature: on_each_way
		}).addTo(map);
	});
	http_get("/geo/stations" + qs, function(data) {
		console.log("Got some data!");
		data = JSON.parse(data);
		if (map.json_layers.stations) {
			map.json_layers.stations.remove();
		}
		map.json_layers.stations = L.geoJSON(data, {
			onEachFeature: on_each_station,
			style: function() {
				return {
					color: "red",
					fillColor: "#f03",
					fillOpacity: 0.5
				};
			}
		}).addTo(map);
	});
}
window.previous_state = null;
window.onload = function() {
	console.log("Loading map...");
	var map = L.map('mappy-map').setView([51.505, -0.09], 13);
	L.tileLayer('http://{s}.tile.osm.org/{z}/{x}/{y}.png', {
		attribution: '&copy; <a href="http://osm.org/copyright">OpenStreetMap</a> contributors'
	}).addTo(map);

	map.json_layers = [];

	function on_map_state_change() {
		if (map.getBounds() != window.previous_state) {
			window.previous_state = map.getBounds();
			console.log("Updating map!");
			update_map(map);
		}
	}
	map.on('moveend', on_map_state_change);
	map.on('resize', on_map_state_change);
	map.on('zoomend', on_map_state_change);
	on_map_state_change();
};
