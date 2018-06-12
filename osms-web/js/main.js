@import '../node_modules/leaflet/dist/leaflet.js';
@import '../node_modules/leaflet-draw/dist/leaflet.draw.js';
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
	fetch("/geo/ways" + qs)
		.then(function(resp) { return resp.json(); })
		.then(function(data) {
			console.log("Got some data!");
			if (map.json_layers.ways) {
				map.json_layers.ways.remove();
			}
			map.json_layers.ways = L.geoJSON(data, {
				onEachFeature: on_each_way
			}).addTo(map);
		})
		.catch(function(err) {
			console.error("Request failed: " + err);
		});
	fetch("/geo/stations" + qs)
		.then(function(resp) { return resp.json(); })
		.then(function(data) {
			console.log("Got some data!");
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
		})
		.catch(function(err) {
			console.error("Request failed: " + err);
		});
}
function confirm_correction(poly, input, div) {
	if (!input.value || input.value.length === 0) {
		alert("Please input a STANOX.");
		return;
	}
	var data = {
		"poly": poly.toGeoJSON(),
		"name": input.value
	};
	fetch("/geo/correct_station", {
		body: JSON.stringify(data),
		headers: {
			'Content-Type': 'application/json'
		},
		method: "POST" 
	})
		.then(function(resp) { 
			if (!resp.ok) {
				return resp.text().then(function(text) { return Promise.reject(text); });
			}
			update_map(window.map);
			poly.remove();
			div.remove();
		})
		.catch(function(err) {
			update_map(window.map);
			alert("Correction failed: " + err);
		});
}
function cancel_correction(poly, div) {
	poly.remove();
	div.remove();
}
function on_correction_polygon(poly) {
	if (poly.layerType != 'polygon') {
		return;
	}
	var layer = poly.layer;
	layer.addTo(window.map);
	var div = document.createElement('div');
	var content = document.createElement('p');
	content.appendChild(document.createTextNode("Which station did you just draw?"));
	content.style['margin-right'] = "10px";
	content.style.display = "inline-block";
	div.appendChild(content);
	var input = document.createElement('input');
	div.appendChild(input);
	var conf = document.createElement('button');
	conf.appendChild(document.createTextNode('Confirm'));
	conf.addEventListener("click", function() {
		confirm_correction(layer, input, div);
	});
	div.appendChild(conf);
	var cancel = document.createElement('button');
	cancel.appendChild(document.createTextNode('Cancel'));
	cancel.addEventListener("click", function() {
		cancel_correction(layer, div);
	});
	div.appendChild(cancel);
	var notifs = document.getElementById('notifs');
	notifs.appendChild(div);
}
window.previous_state = null;
window.onload = function() {
	console.log("Loading map...");
	var map = L.map('mappy-map').setView([51.505, -0.09], 13);
	L.tileLayer('http://{s}.tile.osm.org/{z}/{x}/{y}.png', {
		attribution: '&copy; <a href="http://osm.org/copyright">OpenStreetMap</a> contributors'
	}).addTo(map);
	var drawControl = new L.Control.Draw({
		draw: {
			marker: false,
			feature: false,
			simpleshape: false,
			circle: false,
			circlemarker: false,
			polyline: false,
			rectangle: false,
			toolbar: {
				buttons: {
					polygon: 'Correct station'
				}
			}
		}
	});
	map.addControl(drawControl);
	map.json_layers = [];
	window.map = map;

	function on_map_state_change() {
		if (map.getBounds() != window.previous_state) {
			window.previous_state = map.getBounds();
			console.log("Updating map!");
			update_map(map);
		}
	}
	map.on(L.Draw.Event.CREATED, on_correction_polygon);
	map.on('moveend', on_map_state_change);
	map.on('resize', on_map_state_change);
	map.on('zoomend', on_map_state_change);
	on_map_state_change();
};
