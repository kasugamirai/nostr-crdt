use js_sys::Promise;
use nostr_sdk::{Event, JsonUtil};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
#[cfg(test)]
pub mod test_data {
    //basic test notes
    pub const NOT_NOTE: &str = r#"{"pubkey":"e1ff3bfdd4e40315959b08b4fcc8245eaa514637e1d4ec2ae166b743341be1af","sig":"53087c94115efb071632abc8d514b1f09b20eb8377d2854fa51ab76c4ac0aa6c5766c9af45ded4e2789098bad73117e02d0bdcb96c34866bec1898753a80465a","created_at":1713418044,"id":"0076792624df92e4b0892722c282fdeddd5912e89d61af843e180f2dc02a5530","content":"Pornalreadyfillsthedemandforaigirlfriends.Theaipartonlyaddssomeonetotalkto,whichboysdon'treallycrave.Theaiboyfriendwillbeabiggerdeal,womengetsomeonetotalktoalldaywithoutanythingelse.","kind":6,"tags":[["e","1c556c3a9e892841bef2bfae13ca5fdc50f81054d031a6a16b060a2e5113ae24"],["p","0018b7ee33fb253843639c62e292fec700a69a93b08ee374c5bda971c9b39564"]]}"#;
    pub const REPLY_WITH_MARKER: &str = r#"{"id":"e36817d0509cdd99d854391027bef6f3a0af1d87bdbdb1d9eb73201ff1719e09","kind":1,"pubkey":"77953b3a63bcf1c748dbdeef109bd56de48c30edcd27d2092440c3adca31c975","tags":[["e","39413ed0400101a45abb82dd8949306790234f785ea224717d0f68fa1b36e935","","root"],["e","3cacfcc0afa9d1daf798291b8d8b31fd0b471303f501e188191444ff4cdf1345","","reply"],["p","58ead82fa15b550094f7f5fe4804e0fe75b779dbef2e9b20511eccd69e6d08f9"],["p","460c25e682fda7832b52d1f22d3d22b3176d972f60dcdc3212ed8c92ef85065c"],["p","6e468422dfb74a5738702a8823b9b28168abab8655faacb6853cd0ee15deee93"],["p","77953b3a63bcf1c748dbdeef109bd56de48c30edcd27d2092440c3adca31c975"]],"created_at":1713443749,"content":"Isee.Thanks!","sig":"7b6f820665a7e52b6b655985fbe1287cbd57304b06af68f9d410d0c89e60a69b9c71698fccca7ebeb192d3d004bdb2e1f3eb1fe5352c68a0021cd8d56c1a4218"}"#;
    pub const REPLY_WITH_NO_MARKER: &str = r#"{"content":"Wow how did i only just get this 🤦‍♂️","created_at":1713415596,"id":"0646ee437c5fc88d90a8c9b846edce3611e8a6e8545e952dbd7975f4a52925bb","kind":1,"pubkey":"32e1827635450ebb3c5a7d12c1f8e7b2b514439ac10a67eef3d9fd9c5c68e245","sig":"bff8feafd44078c69402d8d7b3cd5148489d86b8a36ccf28c704920c776b1e568d6556743079866a5d33d70900f3c6fa09e3b0e02bf1f6d7a6a2394873623243","tags":[["e","a200b725177cc2fcbb0c40c5103695da6a8cbd9e73c5a9293c8bfd45521a84bc"],["e","cfab5dabf95fa14c21a611a3eff120132a470201407bd6799ae1c5058b88b430"],["p","79c2cae114ea28a981e7559b4fe7854a473521a8d22a66bbab9fa248eb820ff6"],["p","07457649b3ae6b2c21bee53c5012a2c1f4f6353bb360feebf52959bab4486d67"],["p","deba271e547767bd6d8eec75eece5615db317a03b07f459134b03e7236005655"]]}"#;
    pub const REPLY_TO_ROOT_WITH_NO_MARKER: &str = r#"{"content":"Porn already fills the demand for ai girlfriends. The ai part only adds someone to talk to, which boys don't really crave. The ai boyfriend will be a bigger deal, women get someone to talk to all day without anything else.","created_at":1713418044,"id":"0076792624df92e4b0892722c282fdeddd5912e89d61af843e180f2dc02a5530","kind":1,"pubkey":"e1ff3bfdd4e40315959b08b4fcc8245eaa514637e1d4ec2ae166b743341be1af","sig":"53087c94115efb071632abc8d514b1f09b20eb8377d2854fa51ab76c4ac0aa6c5766c9af45ded4e2789098bad73117e02d0bdcb96c34866bec1898753a80465a","tags":[["e","1c556c3a9e892841bef2bfae13ca5fdc50f81054d031a6a16b060a2e5113ae24"],["p","0018b7ee33fb253843639c62e292fec700a69a93b08ee374c5bda971c9b39564"]]}"#;
    pub const REPLY_TO_ROOT_WITH_MARKER: &str = r#"{"content":"The start???","created_at":1715764169,"id":"fdd7a951ba4e88ca63ea2f9ed77656dbebe78e7039742ab2ab192cde36421933","kind":1,"pubkey":"a6223de378ea5daad05577b87c9c07eda41b171b02465a6e64f9f4356f46025b","sig":"bc444568609aa047c27029800976452f79a75e16b5c2062ecfff61a2eac92ccc69add9fa30d378d15ab1b54e3efbf9d1a6edf22dad8c4031dab794494c696ab3","tags":[["e","ff25d26e734c41fa7ed86d28270628f8fb2f6fb03a23eed3d38502499c1a7a2b","","root"],["p","1bc70a0148b3f316da33fe3c89f23e3e71ac4ff998027ec712b905cd24f6a411"]]}"#;
    pub const ROOT_NOTE: &str = r#"{"content":"If i do createElement and rhen appendChild for a lot of number of time, It took a lot of RAM compared to writting the entire HTML manually.","created_at":1713492656,"id":"c3d8e01d3884d8914583ef1da76e3e1732824228e89cfda3b5fe1164bbb9dd38","kind":1,"pubkey":"347a2370900d19b4e4756221594e8bda706ae5c785de09e59e4605f91a03f49c","sig":"daf83e74d9263c9100c54fa130265b4cfc0d4e659fc596bac6980577c1bebb9fe18681dc3e97898cd0b20fdd3dee70643827a57b90dc08be8177d45bf6e8263e","tags":[]}"#;
    pub const ERROR_EVENT: &str = r#"{"content":"The start???","created_at":1715764169,"id":"fdd7a951ba4e88ca63ea2f9ed77656dbebe78e7039742ab2ab192cde36421933","kind":1,"pubkey":"a6223de378ea5daad05577b87c9c07eda41b171b02465a6e64f9f4356f46025b","sig":"bc444568609aa047c27029800976452f79a75e16b5c2062ecfff61a2eac92ccc69add9fa30d378d15ab1b54e3efbf9d1a6edf22dad8c4031dab794494c696ab3","tags":[["e2","ff25d26e734c41fa7ed86d28270628f8fb2f6fb03a23eed3d38502499c1a7a2b","","root"],["p","1bc70a0148b3f316da33fe3c89f23e3e71ac4ff998027ec712b905cd24f6a411"]]}"#;
    //those events are for reply tree test
    pub const R: &str = r#"{"content":"This is the Root!","created_at":1713517255,"id":"9a708c373de54236d7707feb8c7ae21aa8a204eb9f6dc289de05f90a9e311651","kind":1,"pubkey":"eba1300e9189ef52f89ddd365b8d172d234275b2288c8fbad4a18306ae13562b","sig":"d082581cb2570adc0b0b124e8b72561b22521d7efc8aca28959e7522a55c78c74420cb57440f07ff8ebe741760c417acd0b489c60ff7e4845ea23a3d98414256","tags":[]}"#;
    pub const R_A: &str = r#"{"content":"R -> A","created_at":1713517325,"id":"9421678017349485b5ac0cd8d6de4907f34b00338e8b255c6fcfe6790fb09511","kind":1,"pubkey":"eba1300e9189ef52f89ddd365b8d172d234275b2288c8fbad4a18306ae13562b","sig":"4a84b9e1a0b2e567f2db542aae076f58de854eca4f88e2f2f8fa9fbc8cbdfa6753e39e04481bb7dd6279d7ec427741c679c51468288b5839c50ab1cfea6eaee3","tags":[["e","9a708c373de54236d7707feb8c7ae21aa8a204eb9f6dc289de05f90a9e311651","wss://relay.damus.io/","root"],["e","9a708c373de54236d7707feb8c7ae21aa8a204eb9f6dc289de05f90a9e311651","wss://relay.damus.io/","reply"]]}"#;
    pub const R_A_B: &str = r#"{"content":"R -> A -> B","created_at":1713517509,"id":"b916e11013514ad0d8c5d8005e2c760c4557cc3c261f4f98ec6f1748c7c8b541","kind":1,"pubkey":"eba1300e9189ef52f89ddd365b8d172d234275b2288c8fbad4a18306ae13562b","sig":"cee8db81d4aba889681f25c5358789f2f37da67a39ca7082cdc62c8cabff439f3a2f0f424e86361960169abf4ddb73ee79c7fd4a203a94dbebd8ce477a323b13","tags":[["e","9a708c373de54236d7707feb8c7ae21aa8a204eb9f6dc289de05f90a9e311651","wss://relay.damus.io/","root"],["e","9421678017349485b5ac0cd8d6de4907f34b00338e8b255c6fcfe6790fb09511","wss://relay.damus.io/","reply"]]}"#;
    pub const R_X: &str = r#"{"content":"R -> X","created_at":1713517591,"id":"c1d15b70fb1cb48792cac33949e4daf74148ef58e23a254a947ae11b1a0b89cc","kind":1,"pubkey":"eba1300e9189ef52f89ddd365b8d172d234275b2288c8fbad4a18306ae13562b","sig":"8035bb03c41851be82bae370fcdfafd8af666206b8cd3b2e7788a00d1ef4335c14f919ca4eb7fa3ed1e0614f41f15389d0439099e466dbe9bf0d3fe205269ca5","tags":[["e","9a708c373de54236d7707feb8c7ae21aa8a204eb9f6dc289de05f90a9e311651","","root"],["e","9a708c373de54236d7707feb8c7ae21aa8a204eb9f6dc289de05f90a9e311651","","reply"]]}"#;
    pub const R_Z: &str = r#"{"content":"R -> Z","created_at":1713517740,"id":"e9356a18293d8122c233d19b405ab8523773fa9419db0bd634bd592ebd250a87","kind":1,"pubkey":"eba1300e9189ef52f89ddd365b8d172d234275b2288c8fbad4a18306ae13562b","sig":"5a4c8c02a75b2fb9ffb567995366629d28c2d131b0e5359bbdc008211b400c265384a5d743cedb794526f54f6474ac6151ca02a5ca150a464d0b11840e0c2ffe","tags":[["e","9a708c373de54236d7707feb8c7ae21aa8a204eb9f6dc289de05f90a9e311651","","root"],["e","9a708c373de54236d7707feb8c7ae21aa8a204eb9f6dc289de05f90a9e311651","","reply"]]}"#;
    pub const R_Z_O: &str = r#"{"content":"R -> Z -> O","created_at":1713517783,"id":"b3ec05726a7b456a7a2212981c7278ccb08d366c5caa9d1e29f2b5d652b00cf5","kind":1,"pubkey":"eba1300e9189ef52f89ddd365b8d172d234275b2288c8fbad4a18306ae13562b","sig":"63ea4e6e43006c0dc7501a111eebf348006813d9abb359a317214a6941bb6eceb889b57fca2c57b1deef568f10ca9e3f2105b43da814644612466b04185f7033","tags":[["e","9a708c373de54236d7707feb8c7ae21aa8a204eb9f6dc289de05f90a9e311651","","root"],["e","e9356a18293d8122c233d19b405ab8523773fa9419db0bd634bd592ebd250a87","wss://relay.damus.io/","reply"]]}"#;

    //events from relay
    pub const R_EVENT_770: &str = r#"{"id":"770e3b604de378c67570ce3c521e2fd51c1a59aa85c22ef9aeab7b5f5e2f5e1b","tags":[],"content":"How it started 🤖.......... How it's going 🥜\n\nhttps://m.primal.net/IHQz.png ","created_at":1715871171,"sig":"90a8abf718b28c51e24bce9f95f92250379e6c612937b9f113d2b24dc43492aacdd6c43a220c02e480300a6d84139bfdeb70e2fdd08330f81ef9683b627baf56","pubkey":"50d94fc2d8580c682b071a542f8b1e31a200b0508bab95a33bef0855df281d63","kind":1}"#;
    pub const R_EVENT_70c: &str = r#"{"content":"Not gonna lie the throw back is sexy","created_at":1715871480,"id":"70cfdf05fa80ce6b4a54668788eef31ff7d5a23b74f54943ec9e5a91cb5806f1","kind":1,"pubkey":"3b7fc823611f1aeaea63ee3bf69b25b8aa16ec6e81d1afc39026808fe194354f","sig":"76a90208faa44bbf95d6d9f1100f9667a7f038c32a9b19ac60abe5c4d75a5ba13f74e74fd0830c28920815bd2ac5f2b8ee0bf6b47b32d57b660ab8a7847d5690","tags":[["e","770e3b604de378c67570ce3c521e2fd51c1a59aa85c22ef9aeab7b5f5e2f5e1b","","root"],["p","50d94fc2d8580c682b071a542f8b1e31a200b0508bab95a33bef0855df281d63"]]}"#;
}
#[cfg(test)]
pub mod test_hander {
    use std::sync::Arc;

    use nostr_sdk::prelude::*;
    use wasm_bindgen_test::*;

    use crate::nostr::register::NotificationHandler;

    pub fn create_console_log_handler() -> NotificationHandler {
        Arc::new(|notification| {
            Box::pin(async move {
                match notification {
                    RelayPoolNotification::Message {
                        message: RelayMessage::Event { event, .. },
                        ..
                    } => {
                        console_log!(
                            "eventid: {:?}, author: {:?}, eventkind: {:?}, eventcontent: {:?}",
                            event.id.to_string(),
                            event.author().to_string(),
                            event.kind,
                            event.content
                        );
                        Ok(false) // Return true to stop the handling process
                    }
                    _ => Ok(false),
                }
            })
        })
    }
}

#[cfg(test)]
pub fn event_from(raw: &str) -> Event {
    Event::from_json(raw).unwrap()
}

#[cfg(test)]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window, js_name = setTimeout)]
    fn set_timeout(closure: &Closure<dyn FnMut()>, time: u32) -> i32;
}

#[cfg(test)]
pub async fn sleep(ms: u32) -> Result<(), JsValue> {
    let promise = Promise::new(&mut |resolve, _| {
        let closure = Closure::once(move || {
            resolve.call0(&JsValue::NULL).unwrap();
        });
        set_timeout(&closure, ms);
        // Keep the closure alive until it's called
        closure.forget();
    });
    JsFuture::from(promise).await?;
    Ok(())
}
