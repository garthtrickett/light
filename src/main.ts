//* @refresh reload */

import { invoke } from "https://esm.sh/@tauri-apps/api";
import {
  Event as TauriEvent,
  listen,
} from "https://esm.sh/@tauri-apps/api/event";

import { directives, html, render } from "https://esm.sh/lit-html";
import { map } from "https://esm.sh/lit/directives/map.js";

import "https://esm.sh/@material/web/button/filled-button.js";
import "https://esm.sh/@material/web/textfield/filled-text-field.js";
var view = "";

import copy from 'copy-to-clipboard';


// listen backend-ping event
listen("test-event", function(blah: TauriEvent<any>) {
  alert(blah);
});

// listen backend-ping event
listen("warp-event", function(state: TauriEvent<any>) {
  load_from_state(state.payload, view);
});


function state_tuple_to_array(state_tuple) {
  const state_keys = [
    "account",
    "chats",
    "configuration",
    "counter",
    "friends",
    "identity_exists",
    "logged_in",
  ];
  const state_object = state_keys.reduce(
    (obj, key, index) => ({ ...obj, [key]: state_tuple[index] }),
    {},
  );
  return state_object;
}

let promise_start_sam = invoke("start_sam_command");
promise_start_sam.then(function(result) {
  console.log("blah");
  console.log(JSON.stringify(result));
  console.log("blah");
  result.logged_in = false;

  load_from_state(result, view);
});

let promise_check_for_identity = invoke("check_for_identity_command");
promise_check_for_identity.then(function(result) {
  result.logged_in = false;
  load_from_state(result, view);
});

// let promise_increment_counter = invoke("increment_counter_command", {
//   step: 1,
// });

// promise_increment_counter.then(function (result) {
//   load_from_state(result);
// });

function get_own_did_key() {
  let promise_start_sam = invoke("get_own_did_key_command");
  promise_start_sam.then(function(result) {
    // navigator.clipboard.writeText(result);
    copy(result);
  });
}

function send_login_request(password) {
  let promise_try_login = invoke("login_command", {
    password: password,
  });
  promise_try_login.then(function(result) {
    load_from_state(result, view);
  });
}

function send_friend_request(did_key) {
  let promise = invoke("send_friend_request_command", {
    didKey: did_key,
  });
  promise.then(function(result) {
    load_from_state(result, view);
  });
}
function send_message(conv_id, message) {
  let promise = invoke("send_message_command", {
    convId: conv_id,
    message: message,
  });
  promise.then(function(result) {
    load_from_state(result, view);
  });
}

function send_initial_message(did_key, message) {
  console.log("did_key: " + did_key);
  let promise = invoke("send_initial_message_command", {
    didKey: did_key,
    message: message,
  });
  promise.then(function(result) {
    load_from_state(result, view);
  });
}

function create_identity(username, password) {
  let promise_create_identity = invoke("create_identity_command", {
    username: username,
    password: password,
  });
  promise_create_identity.then(function(result) {
    load_from_state(result, view);
  });
}

function delete_identity() {
  let promise_start_sam = invoke("delete_identity_command", {});
  promise_start_sam.then(function(result) {
    load_from_state(result, view);
  });
}

function set_view_to_individual_chat(friend, state) {
  view = friend;
  load_from_state(state, view);
}

function friends_list_from_state(state, friend_type) {
  var friends_exist = false;
  const friend_list = [];
  for (const friend in state["friends"][friend_type]) {
    friends_exist = true;

    if (friend_type == "all") {
      var button = html`<md-filled-button
                    label="Chat"
                    @click=${() => set_view_to_individual_chat(friend, state)}>
              />`;
    } else if (friend_type == "incoming_requests") {
      var button = html`<md-filled-button
                    label="Accept Request"
                    @click=${() =>
          send_friend_request(friend_values["identity"]["did_key"])}>
              />`;
    } else {
      var button = html``;
    }

    let friend_values = state["friends"][friend_type][friend];
    friend_list.push(
      html`<li> ${friend_values["identity"]["username"]}   
            ${button}
           </li>`,
    );
  }
  return { friend_list, friends_exist };
}

// TODO - ability to send and recieve messages from within an individual chat

function load_from_state(state, view) {
  if (state["identity_exists"] == true) {
    if (state["logged_in"] == true) {
      if (view == "") {
        let friends_div_list = [];
        // shit way to do this but normal object stuff isn't working

        let friend_type_list = [
          "all",
          "incoming_requests",
          "outgoing_requests",
        ];

        for (const friend_type of friend_type_list) {
          var { friend_list, friends_exist } = friends_list_from_state(
            state,
            friend_type,
          );
          if (friends_exist == true) {
            if (friend_type == "all") {
              var heading = "Curators";
            } else if (friend_type == "incoming_requests") {
              var heading = "Incoming Contact Requests";
            } else if (friend_type == "outgoing_requests") {
              var heading = "Outgoing Contact Requests";
            }
            var friends_div = html`<div> ${heading}</div><div> 
        <ul>
        ${friend_list}
        <ul>
        </div>`;
            friends_div_list.push(friends_div);
          }
        }

        var authed_div = html`
      <div>
        <br>
        <br>
        <md-filled-button
              label="Delete identity and terminate"
              @click=${() => delete_identity()}>
        />
      </div>
      <br>
      <br>
      <div>
        <md-filled-button
              label="Copy own did_key to clipboard"
              @click=${() => get_own_did_key()}>
        />
      </div>
      <br>
      <br>
      <div>
                             <md-filled-text-field
                              @change=${(e) => {
            send_friend_request(e.srcElement.value);
            e.srcElement.value = "Friend request failed";
          }}
                              placeholder="Enter did_key to add contact" autofocus />
      </div>    
      <br>
      <br>
   
      ${map(friends_div_list, (friend) => html`<div>${friend}</div>`)}
           `;
      } else {
        var all_chats = state["chats"]["all"];

        var chat_with_this_did_key_exists = false;

        var num_chats = Object.keys(all_chats).length;
        for (let i = 0; i < num_chats; i++) {
          var nth_key = Object.keys(all_chats)[i];
          var nth_chat_did_key =
            all_chats[nth_key]["participants"][1]["identity"]["did_key"];
          if (view == nth_chat_did_key) {
            var chat_with_this_did_key_exists = true;
            var chat = all_chats[nth_key];
          }
        }
        var back_button = html`
        <md-filled-button
                                        label="Back"
                                        @click=${() =>
            load_from_state(state, "")}>
                                  />`;
        if (chat_with_this_did_key_exists == true) {
          // TODO - put in a way to send messages after initial message
          //      - make incoming and outgoing messasges readable here
          var chat_participants = {};

          for (const participants of chat["participants"]) {
            chat_participants[participants["identity"]["did_key"]] =
              participants["identity"]["username"];
          }

          var authed_div = html`<div>
          
                                  ${back_button}
          
      ${map(chat["messages"], (message) =>
            html`<div>${chat_participants[message["sender"]]}: ${message["value"]["0"]
              }</div>`)
            }
                             <md-filled-text-field
                              @change=${(e) => {
              send_message(e.srcElement.value, chat["id"]);
              e.srcElement.value = "";
            }}
                              placeholder="Send message" autofocus />
                          
                                <div>`;
        } else if (chat_with_this_did_key_exists == false) {
          var authed_div = html`Send first message ${back_button}

                             <md-filled-text-field
                              @change=${(e) => {
              send_initial_message(view, e.srcElement.value);
              e.srcElement.value = "";
            }}
                              placeholder="Send first message" autofocus />
                                     
                                `;
        }
      }

      var conditional_child = authed_div;
    } else if (state["logged_in"] == false) {
      const login_div = html`<div>
                           You need to log in
                             <md-filled-text-field
                              @change=${(e) => {
          send_login_request(e.srcElement.value);
          e.srcElement.value = "Password Wrong";
        }}
                              placeholder="Enter Password" autofocus />
                          </div>`;
      var conditional_child = login_div;
    }
  } else {
    const create_identity_div = html`
    <div>
    Username and password must be longer than 4 characters and just use letters
     <form class="form">
        <br>
        <br>
        <md-filled-text-field
              required
              outlined
              name="username"
              label="Username"
              icon="person"
              pattern="^[a-zA-Z0-9-_]+$"
              validationMessage="Please enter alpha numeric characters only."
              @blur=${() => checkFormValidity()}>
        </md-filled-text-field>
        <md-filled-text-field
              required
              outlined
              name="password"
              label="Password"
              icon="person"
              pattern="^[a-zA-Z0-9-_]+$"
              validationMessage="Please enter alpha numeric characters only."
              @blur=${() => checkFormValidity()}>
          </md-filled-text-field>
          <br>
          <br>
          <md-filled-button
              ?disabled=${!true}
              label="Create Identity"
              @click=${() => handleSubmit()}>
          />
        </form>
    </div>`;
    var conditional_child = create_identity_div;
  }
  const root = (state) => html`<div> ${(conditional_child)} </div>`;

  render(root(state), document.body);
}

function handleSubmit() {
  const payload = {};
  const fields = document.body.querySelectorAll(
    "md-filled-text-field",
  );

  fields.forEach((field) => {
    payload[field.name] = field.value;
  });
  if (true) {
    create_identity(payload["username"], payload["password"]);
  }
}

function checkFormValidity() {
  const requiredFields = document.body.querySelectorAll("[required]");
  const validFields = []; // stores the validity of all required fields

  requiredFields.forEach((field) => {
    validFields.push(field.validity.valid);
  });

  // if false is not in the array of validFields, then the form is valid
  let form_validity = !validFields.includes(false);
}
