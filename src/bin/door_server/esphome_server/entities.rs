use std::collections::HashMap;

use esphome_native_api::{
  parser::ProtoMessage,
  proto::version_2025_12_1::{
    EntityCategory, ListEntitiesBinarySensorResponse, ListEntitiesCoverResponse, ListEntitiesEventResponse,
    ListEntitiesLockResponse,
  },
};

pub fn entities() -> HashMap<u32, ProtoMessage> {
  let mut entity_map = HashMap::new();

  let device_id = 0;
  let mut key = 0;

  entity_map.insert(
    key,
    ProtoMessage::ListEntitiesEventResponse(ListEntitiesEventResponse {
      device_id,
      key,
      object_id: "door_bell".into(),
      name: "Door Bell".into(),
      device_class: "doorbell".into(),
      disabled_by_default: false,
      icon: "mdi:doorbell".into(),
      entity_category: EntityCategory::None as i32,
      event_types: vec!["ring".into()],
    }),
  );
  key += 1;

  entity_map.insert(
    key,
    ProtoMessage::ListEntitiesBinarySensorResponse(ListEntitiesBinarySensorResponse {
      device_id,
      key,
      object_id: "main_door".into(),
      name: "Main Door".into(),
      device_class: "door".into(),
      disabled_by_default: false,
      icon: "mdi:door".into(),
      entity_category: EntityCategory::None as i32,
      is_status_binary_sensor: false,
    }),
  );
  key += 1;

  entity_map.insert(
    key,
    ProtoMessage::ListEntitiesEventResponse(ListEntitiesEventResponse {
      device_id,
      key,
      object_id: "main_door".into(),
      name: "Main Door".into(),
      device_class: "".into(),
      disabled_by_default: false,
      icon: "mdi:fingerprint".into(),
      entity_category: EntityCategory::None as i32,
      event_types: vec!["open".into()],
    }),
  );
  key += 1;

  entity_map.insert(
    key,
    ProtoMessage::ListEntitiesLockResponse(ListEntitiesLockResponse {
      device_id,
      key,
      object_id: "main_door".into(),
      name: "Main Door".into(),
      disabled_by_default: false,
      icon: "mdi:door-closed-lock".into(),
      entity_category: EntityCategory::None as i32,
      assumed_state: true,
      supports_open: false,
      requires_code: false,
      code_format: "".into(),
    }),
  );
  key += 1;

  entity_map.insert(
    key,
    ProtoMessage::ListEntitiesBinarySensorResponse(ListEntitiesBinarySensorResponse {
      device_id,
      key,
      object_id: "cellar_door".into(),
      name: "Cellar Door".into(),
      device_class: "door".into(),
      disabled_by_default: false,
      icon: "mdi:door".into(),
      entity_category: EntityCategory::None as i32,
      is_status_binary_sensor: false,
    }),
  );
  key += 1;

  entity_map.insert(
    key,
    ProtoMessage::ListEntitiesEventResponse(ListEntitiesEventResponse {
      device_id,
      key,
      object_id: "cellar_door".into(),
      name: "Cellar Door".into(),
      device_class: "".into(),
      disabled_by_default: false,
      icon: "mdi:fingerprint".into(),
      entity_category: EntityCategory::None as i32,
      event_types: vec!["open".into()],
    }),
  );
  key += 1;

  entity_map.insert(
    key,
    ProtoMessage::ListEntitiesLockResponse(ListEntitiesLockResponse {
      device_id,
      key,
      object_id: "cellar_door".into(),
      name: "Cellar Door".into(),
      disabled_by_default: false,
      icon: "mdi:door-closed-lock".into(),
      entity_category: EntityCategory::None as i32,
      assumed_state: true,
      supports_open: false,
      requires_code: false,
      code_format: "".into(),
    }),
  );
  key += 1;

  entity_map.insert(
    key,
    ProtoMessage::ListEntitiesBinarySensorResponse(ListEntitiesBinarySensorResponse {
      device_id,
      key,
      object_id: "garage_door".into(),
      name: "Garage Door".into(),
      device_class: "garage_door".into(),
      disabled_by_default: false,
      icon: "mdi:garage-variant".into(),
      entity_category: EntityCategory::None as i32,
      is_status_binary_sensor: false,
    }),
  );
  key += 1;

  entity_map.insert(
    key,
    ProtoMessage::ListEntitiesEventResponse(ListEntitiesEventResponse {
      device_id,
      key,
      object_id: "garage_door".into(),
      name: "Garage Door".into(),
      device_class: "".into(),
      disabled_by_default: false,
      icon: "mdi:fingerprint".into(),
      entity_category: EntityCategory::None as i32,
      event_types: vec!["open".into()],
    }),
  );
  key += 1;

  entity_map.insert(
    key,
    ProtoMessage::ListEntitiesCoverResponse(ListEntitiesCoverResponse {
      device_id,
      key,
      object_id: "garage_door".into(),
      name: "Garage Door".into(),
      device_class: "garage".into(),
      disabled_by_default: false,
      icon: "mdi:garage-variant".into(),
      entity_category: EntityCategory::None as i32,
      assumed_state: true,
      supports_position: true,
      supports_tilt: false,
      supports_stop: true,
    }),
  );

  entity_map
}
